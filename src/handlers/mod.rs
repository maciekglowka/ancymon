use async_trait::async_trait;

use crate::{errors::AncymonError, events::EventMeta, values::Value};

pub mod discord;
pub mod ical;
pub mod sql;

pub trait HandlerBuilder {
    fn build(&self) -> Result<Box<dyn EventHandler + Send + Sync>, AncymonError>;
}
impl<T> HandlerBuilder for T
where
    T: EventHandler + Clone + Send + Sync + 'static,
{
    fn build(&self) -> Result<Box<dyn EventHandler + Send + Sync>, AncymonError> {
        Ok(Box::new(self.clone()))
    }
}

#[async_trait]
pub trait EventHandler {
    #[allow(unused_variables)]
    async fn init(&mut self, config: &Value) -> Result<(), AncymonError> {
        Ok(())
    }
    async fn execute(
        &self,
        event: &Value,
        arguments: &Value,
        meta: &mut EventMeta,
    ) -> Result<Value, AncymonError>;
}

#[derive(Clone)]
pub struct DebugHandler;
#[async_trait]
impl EventHandler for DebugHandler {
    async fn execute(
        &self,
        event: &Value,
        _arguments: &Value,
        _: &mut EventMeta,
    ) -> Result<Value, AncymonError> {
        tracing::debug!("{event:?}");
        Ok(event.clone())
    }
}
