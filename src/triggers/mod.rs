use async_trait::async_trait;
use chrono::Utc;

use crate::{errors::AncymonError, events::Event, Value};

pub mod cron;
pub mod discord;

#[derive(Clone, Debug)]
pub struct Trigger {
    pub source: String,
    pub(crate) emit: String,
    pub(crate) arguments: Value,
}
impl mlua::FromLua for Trigger {
    fn from_lua(value: mlua::Value, _: &mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Table(t) => Ok(Trigger {
                source: t.get("source")?,
                emit: t.get("emit")?,
                arguments: t.get("arguments").unwrap_or_default(),
            }),
            v => Err(mlua::Error::FromLuaConversionError {
                // FIXME
                from: std::any::type_name_of_val(&v),
                to: "Trigger".to_string(),
                message: Some("Expected table".to_string()),
            }),
        }
    }
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
