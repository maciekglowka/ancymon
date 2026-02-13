use async_trait::async_trait;

use crate::{errors::AncymonError, events::EventValue, values::Value};

pub mod discord;
pub mod sql;

pub trait HandlerBuilder {
    fn build(&self) -> Result<Box<dyn EventHandler + Send + Sync>, AncymonError>;
}

#[async_trait]
pub trait EventHandler {
    async fn init(&mut self, config: &toml::Table) -> Result<(), AncymonError> {
        Ok(())
    }
    async fn execute(&self, event: &Value, arguments: &Value) -> EventValue;
}

pub struct DebugHandler;
#[async_trait]
impl EventHandler for DebugHandler {
    async fn execute(&self, event: &Value, _arguments: &Value) -> EventValue {
        println!("{event:?}");
        Ok(event.clone())
    }
}

pub struct DebugBuilder;
impl HandlerBuilder for DebugBuilder {
    fn build(&self) -> Result<Box<dyn EventHandler + Send + Sync>, AncymonError> {
        Ok(Box::new(DebugHandler))
    }
}
