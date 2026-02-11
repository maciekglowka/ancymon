use async_trait::async_trait;

use crate::errors::AncymonError;

pub mod discord;
pub mod sql;

pub trait HandlerBuilder {
    fn build(&self) -> Result<Box<dyn EventHandler + Send>, AncymonError>;
}

#[async_trait]
pub trait EventHandler {
    async fn init(&mut self, config: &toml::Table) -> Result<(), AncymonError> {
        Ok(())
    }
    async fn execute(
        &self,
        event: Option<&toml::Value>,
        arguments: &toml::Value,
    ) -> Result<Option<toml::Value>, AncymonError>;
}

pub struct DebugHandler;
#[async_trait]
impl EventHandler for DebugHandler {
    async fn execute(
        &self,
        event: Option<&toml::Value>,
        _arguments: &toml::Value,
    ) -> Result<Option<toml::Value>, AncymonError> {
        println!("{event:?}");
        Ok(None)
    }
}

pub struct DebugBuilder;
impl HandlerBuilder for DebugBuilder {
    fn build(&self) -> Result<Box<dyn EventHandler + Send>, AncymonError> {
        Ok(Box::new(DebugHandler))
    }
}
