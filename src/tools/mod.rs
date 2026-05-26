use async_trait::async_trait;

use crate::{errors::AncymonError, values::Value};

pub mod discord;
pub mod ical;
pub mod smtp;
pub mod sql;

pub trait ToolBuilder {
    fn build(&self) -> Result<Box<dyn Tool + Send + Sync>, AncymonError>;
}
impl<T> ToolBuilder for T
where
    T: Tool + Clone + Send + Sync + 'static,
{
    fn build(&self) -> Result<Box<dyn Tool + Send + Sync>, AncymonError> {
        Ok(Box::new(self.clone()))
    }
}

#[async_trait]
pub trait Tool {
    #[allow(unused_variables)]
    async fn init(&mut self, config: &Value) -> Result<(), AncymonError> {
        Ok(())
    }
    async fn execute(&self, arguments: &Value) -> Result<Value, AncymonError>;
}

#[derive(Clone)]
pub struct DebugPrint;
#[async_trait]
impl Tool for DebugPrint {
    async fn execute(&self, arguments: &Value) -> Result<Value, AncymonError> {
        tracing::debug!("{arguments:?}");
        Ok(Value::Null)
    }
}
