use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::str::FromStr;

use crate::{
    errors::{AncymonError, BuildError, ConfigError},
    events::Event,
    triggers::{Trigger, TriggerSource},
};

#[derive(Default)]
pub struct CronTrigger {
    schedules: Vec<cron::Schedule>,
    triggers: Vec<Trigger>,
}
impl CronTrigger {
    /// Return next scheduled time + trigger indices to fire
    fn next(&self) -> (DateTime<Utc>, Vec<usize>) {
        let mut upcoming = self
            .schedules
            .iter()
            .map(|a| a.upcoming(Utc).take(1).next().unwrap())
            .enumerate()
            .collect::<Vec<_>>();
        upcoming.sort_by_key(|a| a.1);
        let indices = upcoming
            .iter()
            .filter(|a| a.1 == upcoming[0].1)
            .map(|a| a.0)
            .collect();

        (upcoming[0].1, indices)
    }
}

#[async_trait]
impl TriggerSource for CronTrigger {
    async fn init(
        &mut self,
        config: &toml::Table,
        triggers: Vec<Trigger>,
    ) -> Result<(), AncymonError> {
        if triggers.is_empty() {
            return Err(ConfigError::MissingValue("No cron triggers specified".to_string()).into());
        }

        // Make sure schedules and triggers are synced.
        self.schedules.clear();

        for trigger in triggers.iter() {
            let pat = trigger
                .arguments
                .as_str()
                .ok_or(ConfigError::InvalidValueType(
                    "Cron arguments: expected string".to_string(),
                ))?;
            let schedule = cron::Schedule::from_str(pat).unwrap();
            self.schedules.push(schedule);
        }

        self.triggers = triggers;

        Ok(())
    }
    async fn run(&mut self, tx: tokio::sync::mpsc::Sender<Event>) {
        loop {
            let (deadline, indices) = self.next();

            // TODO check precision
            let duration = deadline - Utc::now();
            // FIXME might panic if duration equals 0
            tokio::time::sleep(duration.to_std().unwrap()).await;
            let value = toml::Value::Integer(deadline.timestamp());
            for i in indices {
                tx.send(Event::new(
                    self.triggers[i].emit.to_string(),
                    Ok(Some(value.clone())),
                ))
                .await
                .unwrap();
            }
        }
    }
}
