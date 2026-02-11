use async_trait::async_trait;

use crate::errors::AncymonError;

pub mod sql;

#[async_trait]
pub trait QuerySource {
    async fn init(&mut self, config: &toml::Table) -> Result<(), AncymonError>;
    async fn execute(&self, arguments: &toml::Value) -> Result<String, AncymonError>;
}
