use async_trait::async_trait;
use chrono::{DateTime, Local};
use std::{collections::BinaryHeap, str::FromStr};

use crate::{
    errors::{AncymonError, ConfigError},
    events::Event,
    triggers::{Trigger, TriggerSource},
    values::Value,
};

#[derive(Default)]
pub struct CronTrigger {
    entries: BinaryHeap<CronEntry>,
}

#[async_trait]
impl TriggerSource for CronTrigger {
    async fn init(&mut self, _: &Value, triggers: Vec<Trigger>) -> Result<(), AncymonError> {
        if triggers.is_empty() {
            return Err(ConfigError::MissingValue("No cron triggers specified".to_string()).into());
        }

        self.entries =
            triggers
                .into_iter()
                .map(|t| {
                    let pat = t.arguments.as_str().ok_or(ConfigError::InvalidValueType(
                        "Cron arguments: expected string".to_string(),
                    ))?;

                    let schedule = cron::Schedule::from_str(pat).unwrap();
                    let next = schedule.upcoming(Local).take(1).next().ok_or(
                        ConfigError::InvalidValue(format!(
                            "Invalid cron value at {:?}",
                            t.arguments
                        )),
                    )?;
                    Ok(CronEntry(next, schedule, t))
                })
                .collect::<Result<BinaryHeap<CronEntry>, AncymonError>>()?;

        Ok(())
    }
    async fn run(&mut self, tx: tokio::sync::mpsc::Sender<Event>) {
        while let Some(entry) = self.entries.pop() {
            let now = Local::now();
            if entry.0 > now {
                tokio::time::sleep((entry.0 - now).to_std().unwrap()).await;
            }
            tx.send(Event::new(
                entry.2.emit.to_string(),
                Ok(Value::Integer(entry.0.timestamp())),
            ))
            .await
            .unwrap();

            if let Some(next) = entry.1.upcoming(Local).take(1).next() {
                self.entries.push(CronEntry(next, entry.1, entry.2));
            }
        }
    }
}

struct CronEntry(DateTime<Local>, cron::Schedule, Trigger);
impl Ord for CronEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse ordering on `next`
        other.0.cmp(&self.0)
    }
}
impl PartialOrd for CronEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // Use Ord's method
        Some(self.cmp(other))
    }
}
impl PartialEq for CronEntry {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for CronEntry {}
