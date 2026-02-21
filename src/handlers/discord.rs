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
pub struct DiscordDmHandler {
    http: Option<Http>,
    channel: PrivateChannel,
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
        let mut content = arguments.message.to_string();
        if arguments.include_event {
            content += "\n";
            content += &event.pretty();
        }
        let msg = self
            .channel
            .send_message(
                self.http.as_ref().unwrap(),
                CreateMessage::new().content(content),
            )
            .await
            .map_err(|e| RuntimeError::Handler(format!("Discord DM sending failed: {e}")))?;
        Ok(Value::String(msg.content))
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
