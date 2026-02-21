use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use crate::{errors::AncymonError, events::Event, Value};

pub mod cron;
pub mod discord;

#[derive(Clone, Debug, Deserialize)]
pub struct Trigger {
    pub source: String,
    pub(crate) emit: String,
    #[serde(default)]
    pub(crate) arguments: Value,
}

#[async_trait]
pub trait TriggerSource {
    async fn init(&mut self, config: &Value, triggers: Vec<Trigger>) -> Result<(), AncymonError>;
    async fn run(&mut self, tx: tokio::sync::mpsc::Sender<Event>);
}

#[derive(Default)]
pub struct StartupTrigger(Vec<Trigger>);
#[async_trait]
impl TriggerSource for StartupTrigger {
    async fn init(&mut self, _: &Value, triggers: Vec<Trigger>) -> Result<(), AncymonError> {
        self.0 = triggers;
        Ok(())
    }
    async fn run(&mut self, tx: tokio::sync::mpsc::Sender<Event>) {
        let ts = Utc::now().timestamp();
        for trigger in self.0.iter() {
            tx.send(Event::initial(
                trigger.emit.to_string(),
                Ok(Value::Integer(ts)),
            ))
            .await
            .unwrap();
        }
    }
}
