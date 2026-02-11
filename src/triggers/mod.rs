use async_trait::async_trait;
use serde::Deserialize;

use crate::{errors::AncymonError, events::Event};

pub mod cron;
pub mod discord;

#[derive(Clone, Debug, Deserialize)]
pub struct Trigger {
    pub source: String,
    pub(crate) emit: String,
    pub(crate) arguments: toml::Value,
}

#[async_trait]
pub trait TriggerSource {
    async fn init(
        &mut self,
        config: &toml::Table,
        triggers: Vec<Trigger>,
    ) -> Result<(), AncymonError>;
    async fn run(&mut self, tx: tokio::sync::mpsc::Sender<Event>);
}
