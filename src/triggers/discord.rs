use std::collections::HashMap;

use async_trait::async_trait;
use serde::Deserialize;
use serenity::all::{
    ClientBuilder, Context, EventHandler as DiscordHandler, GatewayIntents, Message, Ready,
};
use tokio::sync::mpsc::{Receiver, Sender, channel};

use crate::{
    errors::{AncymonError, BuildError, ConfigError},
    events::Event,
    triggers::{Trigger, TriggerSource},
    values::Value,
};

/// A trigger source that listens for command messages in a Discord channel.
///
/// Each command must be registered in the config file as a trigger argument.
/// It has to start with a `!` prefix and can contain an extra string payload.
/// E.g. `!print some content`.
///
/// Output value
///
/// Trigger emits either Value::String event, if payload was found or Value::Null otherwise.
/// Additionaly event meta will contain a "discord_original_user_id" string field,
/// containing Discord Id of the user that send the trigger message.
///
/// Usage Example (please note that Ancymon supports env variables expansion):
/// ```toml
/// [sources.discord-command]
/// type = "discord-command"
/// bot-token = "${DISCORD_TOKEN}"
///
/// [[triggers]]
/// source = "discord-command"
/// emit = "debug-trigger"
/// arguments = "!debug"
/// ```
#[derive(Default)]
pub struct DiscordCommandTrigger {
    cmd_rx: Option<Receiver<Command>>,
}

#[async_trait]
impl TriggerSource for DiscordCommandTrigger {
    async fn init(&mut self, config: &Value, triggers: Vec<Trigger>) -> Result<(), AncymonError> {
        let config: DiscordCommandConfig = config.clone().try_into()?;
        let intents = GatewayIntents::DIRECT_MESSAGES;

        let mut entries = HashMap::new();

        for trigger in triggers {
            let command = trigger
                .arguments
                .as_str()
                .ok_or(ConfigError::InvalidValueType(format!(
                    "Expected string for Discord command, found: {:?}",
                    trigger.arguments
                )))?;
            let entry = CommandEntry { emit: trigger.emit };
            entries.insert(command.to_string(), entry);
        }

        let (cmd_tx, cmd_rx) = channel(32);
        self.cmd_rx = Some(cmd_rx);

        let handler = CommandHandler { cmd_tx, entries };

        let mut client = ClientBuilder::new(&config.bot_token, intents)
            .event_handler(handler)
            .await
            .map_err(|e| {
                BuildError::Source(format!("Failed to create Discord command source: {e}"))
            })?;

        // FIXME take handle to cancel task on shutdown
        tokio::spawn(async move { client.start().await.unwrap() });

        Ok(())
    }
    async fn run(&mut self, tx: tokio::sync::mpsc::Sender<Event>) {
        while let Some(command) = self.cmd_rx.as_mut().unwrap().recv().await {
            let mut event = Event::initial(command.emit, Ok(command.content));
            event.meta.insert(
                "discord_original_user_id".to_string(),
                Value::String(command.user_id),
            );
            tx.send(event).await.unwrap();
        }
    }
}

struct Command {
    emit: String,
    content: Value,
    // String is used as u64 might overflow Value's i64
    user_id: String,
}

struct CommandEntry {
    emit: String,
}

#[derive(Deserialize)]
struct DiscordCommandConfig {
    #[serde(rename = "bot-token")]
    bot_token: String,
}

pub struct CommandHandler {
    entries: HashMap<String, CommandEntry>,
    cmd_tx: Sender<Command>,
}

#[async_trait]
impl DiscordHandler for CommandHandler {
    async fn message(&self, _: Context, msg: Message) {
        if msg.content.chars().next() != Some('!') {
            return;
        };
        let (command, payload) = match msg.content.split_once(' ') {
            // Has string payload.
            Some((command, tail)) => (command, Value::String(tail.to_string())),
            // Just the command.
            None => (msg.content.as_str(), Value::Null),
        };
        if let Some(entry) = self.entries.get(command) {
            self.cmd_tx
                .send(Command {
                    emit: entry.emit.to_string(),
                    content: payload,
                    user_id: msg.author.id.to_string(),
                })
                .await
                .unwrap();
            //
        } else {
            tracing::warn!("Unknown command: {command}");
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        tracing::info!("Discord Command: {} is connected!", ready.user.name);
    }
}
