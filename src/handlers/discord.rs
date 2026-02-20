use async_trait::async_trait;
use serenity::all::Http;

use crate::{
    errors::AncymonError,
    handlers::{EventHandler, HandlerBuilder},
    shared::discord::get_http,
    values::Value,
};

/// Sends DM messages to selected user.
pub struct DiscordDmHandler {
    http: Http,
    user_id: u64,
}
#[async_trait]
impl EventHandler for DiscordDmHandler {
    async fn init(&mut self, config: &Value) -> Result<(), AncymonError> {
        // self.user_id = config
        //     .get("user-id")
        //     .ok_or(ConfigError::MissingValue(format!("")))
        Ok(())
    }
    async fn execute(&self, event: &Value, _arguments: &Value) -> Result<Value, AncymonError> {
        tracing::debug!("{event:?}");
        Ok(event.clone())
    }
}

pub struct DiscordDmBuilder;
impl HandlerBuilder for DiscordDmBuilder {
    fn build(&self) -> Result<Box<dyn EventHandler + Send + Sync>, AncymonError> {
        Ok(Box::new(DiscordDmHandler {
            http: get_http()?,
            user_id: 0,
        }))
    }
}
