use std::collections::HashMap;

use async_trait::async_trait;
use serde::Deserialize;
use serenity::all::{
    ClientBuilder, Context, EventHandler as DiscordHandler, GatewayIntents, Message, Ready,
};
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::{
    errors::{AncymonError, BuildError, ConfigError},
    events::Event,
    triggers::{Trigger, TriggerSource},
    values::Value,
};

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
            tx.send(Event::new(command.emit, Ok(command.content)))
                .await
                .unwrap();
        }
    }
}

struct Command {
    emit: String,
    content: Value,
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
        let Some((command, tail)) = msg.content.split_once(' ') else {
            tracing::debug!("Could not parse message as command: {msg:?}");
            return;
        };
        if let Some(entry) = self.entries.get(command) {
            self.cmd_tx
                .send(Command {
                    emit: entry.emit.to_string(),
                    content: Value::String(tail.to_string()),
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
