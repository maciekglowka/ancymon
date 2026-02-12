use std::collections::{HashMap, VecDeque};

use crate::{
    actions::{AcceptedInput, Action},
    config::Config,
    errors::{AncymonError, ConfigError},
    events::Event,
    handlers::{EventHandler, HandlerBuilder},
    triggers::{Trigger, TriggerSource},
};

const RECV_BUFFER_SIZE: usize = 10;

pub struct Bot {
    actions: HashMap<String, Vec<Action>>,
    handlers: HashMap<String, Box<dyn EventHandler + Send>>,
    sources: Vec<Box<dyn TriggerSource + Send>>,
}
impl Bot {
    pub async fn run(&mut self) -> Result<(), AncymonError> {
        tracing::info!("Bot is starting...");
        let (tx, mut rx) = tokio::sync::mpsc::channel(256);

        self.spawn_sources(tx).await;
        let mut queue = VecDeque::new();
        let mut buf = Vec::with_capacity(RECV_BUFFER_SIZE);

        loop {
            while let Some(event) = queue.pop_front() {
                // TODO parallelize
                queue.extend(self.execute_event(&event).await);
                tracing::info!("Executing event {}", event.name);
            }
            // If the queue is empty then wait for a trigger.
            let received = rx.recv_many(&mut buf, RECV_BUFFER_SIZE).await;
            if received == 0 {
                // No more senders
                break;
            }
            queue.extend(buf.drain(..received));
        }
        Ok(())
    }

    async fn spawn_sources(&mut self, tx: tokio::sync::mpsc::Sender<Event>) {
        for mut source in self.sources.drain(..) {
            // TODO take join handle
            let source_tx = tx.clone();
            tokio::spawn(async move { source.run(source_tx).await });
        }
    }

    async fn execute_event(&self, event: &Event) -> Vec<Event> {
        let mut events = Vec::new();

        for action in self.actions.get(&event.name).iter().cloned().flatten() {
            let Some(handler) = self.handlers.get(&action.handler) else {
                tracing::error!("Handler not found: {}", action.handler);
                continue;
            };

            let result = match (&event.value, action.accepted_input) {
                (Ok(Some(v)), AcceptedInput::Some) => {
                    Some(handler.execute(Some(v), &action.arguments).await)
                }
                (Ok(None), AcceptedInput::None) => {
                    Some(handler.execute(None, &action.arguments).await)
                }
                (Ok(v), AcceptedInput::Ok) => {
                    Some(handler.execute(v.as_ref(), &action.arguments).await)
                }
                (Err(e), AcceptedInput::Err) => Some(
                    handler
                        .execute(
                            Some(&toml::Value::String(format!("{e}"))),
                            &action.arguments,
                        )
                        .await,
                ),
                _ => None,
            };
            if let Some(result) = result {
                events.push(Event::new(action.emit.to_string(), result));
            }
        }
        events
    }
}

#[derive(Default)]
pub struct BotBuilder {
    handler_builders: HashMap<String, Box<dyn HandlerBuilder>>,
    trigger_sources: HashMap<String, Box<dyn TriggerSource + Send>>,
}
impl BotBuilder {
    pub async fn build(mut self, config: Config) -> Result<Bot, AncymonError> {
        let handlers = self.build_handlers(&config).await?;
        let actions = self.build_actions(&config).await?;
        self.init_trigger_sources(&config).await?;

        Ok(Bot {
            actions,
            handlers,
            sources: self.trigger_sources.into_values().collect(),
        })
    }
    pub fn with_handler<T: HandlerBuilder + 'static>(
        mut self,
        name: impl Into<String>,
        builder: T,
    ) -> Self {
        self.handler_builders
            .insert(name.into(), Box::new(builder) as Box<dyn HandlerBuilder>);
        self
    }

    pub fn with_source<T: TriggerSource + Send + 'static>(
        mut self,
        name: impl Into<String>,
        source: T,
    ) -> Self {
        self.trigger_sources.insert(
            name.into(),
            Box::new(source) as Box<dyn TriggerSource + Send>,
        );
        self
    }

    async fn build_handlers(
        &self,
        config: &Config,
    ) -> Result<HashMap<String, Box<dyn EventHandler + Send>>, AncymonError> {
        let mut handlers = HashMap::new();

        for (name, handler_config) in config.handlers.iter() {
            let builder = handler_config
                .get("type")
                .ok_or(ConfigError::MissingValue(format!(
                    "Key not found: `type` at handler config {name}"
                )))?
                .as_str()
                .ok_or(ConfigError::InvalidValueType(format!(
                    "Expected string for key `type` at handler config {name}"
                )))?;
            let mut handler = self
                .handler_builders
                .get(builder)
                .ok_or(ConfigError::InvalidHandlerType(builder.to_string()))?
                .build()?;
            handler.init(handler_config).await?;
            handlers.insert(name.to_string(), handler);
        }
        Ok(handlers)
    }

    async fn build_actions(
        &self,
        config: &Config,
    ) -> Result<HashMap<String, Vec<Action>>, AncymonError> {
        let mut actions: HashMap<String, Vec<Action>> = HashMap::new();

        for action in config.actions.iter() {
            if let Some(event) = actions.get_mut(&action.event) {
                event.push(action.clone());
                continue;
            }
            actions.insert(action.event.to_string(), vec![action.clone()]);
        }
        Ok(actions)
    }

    async fn init_trigger_sources(&mut self, config: &Config) -> Result<(), AncymonError> {
        let mut triggers: HashMap<String, Vec<Trigger>> = HashMap::new();

        for trigger in config.triggers.iter() {
            if let Some(entry) = triggers.get_mut(&trigger.source) {
                entry.push(trigger.clone());
                continue;
            }
            triggers.insert(trigger.source.to_string(), vec![trigger.clone()]);
        }

        for (source_name, triggers) in triggers {
            let source = self
                .trigger_sources
                .get_mut(&source_name)
                .ok_or(ConfigError::InvalidSource(source_name.to_string()))?;

            source
                .init(
                    config
                        .sources
                        .get(&source_name)
                        .ok_or(ConfigError::MissingConfig(source_name))?,
                    triggers,
                )
                .await?;
        }

        Ok(())
    }
}
