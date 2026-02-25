use async_trait::async_trait;
use serde::Deserialize;
use serenity::all::{CreateMessage, Http, PrivateChannel, UserId};

use crate::{
    errors::{AncymonError, BuildError, ConfigError, RuntimeError},
    events::EventMeta,
    handlers::{EventHandler, HandlerBuilder},
    values::Value,
};

/// Sends DM messages to selected user.
///
/// The base message content comes from `arguments.message` field in the config.
/// If `arguments.include-event` field is set to true, the contents of the
/// triggering event is also included as a subsequent message(s).
/// If the event value is of a string type, it will be send as it is.
/// If the event value is an array, it will be send as a series of messages.
/// Otherwise the value will be sent pretty printed.
pub struct DiscordDmHandler {
    http: Option<Http>,
    channel: PrivateChannel,
}
impl DiscordDmHandler {
    async fn send_single(&self, content: &str) -> Result<(), AncymonError> {
        let _ = self
            .channel
            .send_message(
                self.http.as_ref().unwrap(),
                CreateMessage::new().content(content),
            )
            .await
            .map_err(|e| RuntimeError::Handler(format!("Discord DM sending failed: {e}")))?;
        Ok(())
    }
}
#[async_trait]
impl EventHandler for DiscordDmHandler {
    async fn init(&mut self, config: &Value) -> Result<(), AncymonError> {
        let config: DiscordDmConfig = config.clone().try_into()?;

        self.http = Some(Http::new(&config.bot_token));

        let user_id = config.user_id.parse().map_err(|e| {
            ConfigError::InvalidValue(format!("Discord user id cannot be parsed: {e}"))
        })?;
        let user_id = UserId::new(user_id);
        self.channel = user_id
            .create_dm_channel(self.http.as_ref().unwrap())
            .await
            .map_err(|e| {
                BuildError::Handler(format!("Cannot create Discord private channel: {e}"))
            })?;
        Ok(())
    }
    async fn execute(
        &self,
        event: &Value,
        arguments: &Value,
        _: &mut EventMeta,
    ) -> Result<Value, AncymonError> {
        let arguments: DiscordDmArguments = arguments.clone().try_into()?;

        self.send_single(&arguments.message).await?;

        if !arguments.include_event {
            return Ok(Value::String(arguments.message.to_string()));
        }

        // Handle sending event content.

        let mut sent_content = vec![Value::String(arguments.message.to_string())];

        match event {
            Value::Array(a) => {
                for entry in a.iter() {
                    self.send_single(&entry.pretty()).await?;
                    sent_content.push(Value::String(entry.pretty()));
                }
            }
            _ => {
                self.send_single(&event.pretty()).await?;
                sent_content.push(Value::String(event.pretty()));
            }
        }

        Ok(Value::Array(sent_content))
    }
}

pub struct DiscordDmBuilder;
impl HandlerBuilder for DiscordDmBuilder {
    fn build(&self) -> Result<Box<dyn EventHandler + Send + Sync>, AncymonError> {
        Ok(Box::new(DiscordDmHandler {
            http: None,
            channel: PrivateChannel::default(),
        }))
    }
}

#[derive(Deserialize)]
struct DiscordDmConfig {
    #[serde(rename = "bot-token")]
    bot_token: String,
    #[serde(rename = "user-id")]
    /// String is used as the id will most likely come from env variable.
    user_id: String,
}

#[derive(Deserialize)]
struct DiscordDmArguments {
    message: String,
    #[serde(default)]
    #[serde(rename = "include-event")]
    include_event: bool,
}
