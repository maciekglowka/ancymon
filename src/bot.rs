use std::{collections::HashMap, sync::Arc};

use tokio::sync::mpsc::{Receiver, Sender};

use crate::{
    actions::{AcceptedInput, Action},
    config::Config,
    errors::{AncymonError, ConfigError},
    events::Event,
    handlers::{EventHandler, HandlerBuilder},
    triggers::{Trigger, TriggerSource},
    values::Value,
};

const QUEUE_SIZE: usize = 256;

struct BotContext {
    actions: HashMap<String, Vec<Action>>,
    handlers: HashMap<String, Box<dyn EventHandler + Send + Sync>>,
    tx: Sender<Event>,
}

#[derive(Default)]
pub struct Bot {
    handler_builders: HashMap<String, Box<dyn HandlerBuilder>>,
    trigger_sources: HashMap<String, Box<dyn TriggerSource + Send + Sync>>,
}
impl Bot {
    pub async fn run(mut self, config: Config) -> Result<(), AncymonError> {
        let handlers = self.build_handlers(&config).await?;
        let actions = self.build_actions(&config).await?;

        self.init_trigger_sources(&config).await?;
        let sources = self.trigger_sources.into_values().collect();

        let (tx, rx) = tokio::sync::mpsc::channel(QUEUE_SIZE);

        let context = BotContext {
            actions,
            handlers,
            tx: tx.clone(),
        };

        spawn_sources(sources, tx).await;
        run(context, rx).await?;

        Ok(())
    }
    pub fn with_handler_type<T: HandlerBuilder + 'static>(
        mut self,
        name: impl Into<String>,
        builder: T,
    ) -> Self {
        self.handler_builders
            .insert(name.into(), Box::new(builder) as Box<dyn HandlerBuilder>);
        self
    }

    pub fn with_source_type<T: TriggerSource + Send + Sync + 'static>(
        mut self,
        name: impl Into<String>,
        source: T,
    ) -> Self {
        self.trigger_sources.insert(
            name.into(),
            Box::new(source) as Box<dyn TriggerSource + Send + Sync>,
        );
        self
    }

    async fn build_handlers(
        &self,
        config: &Config,
    ) -> Result<HashMap<String, Box<dyn EventHandler + Send + Sync>>, AncymonError> {
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

async fn run(context: BotContext, mut rx: Receiver<Event>) -> Result<(), AncymonError> {
    tracing::info!("Ancymon Bot is starting...");
    let context = Arc::new(context);

    while let Some(event) = rx.recv().await {
        tracing::info!("Executing event: {}", event.name);
        // TODO add concurrent events limit? (tokio::Semaphore?)
        let event_context = Arc::clone(&context);
        tokio::spawn(execute_event(event, event_context));
    }

    Ok(())
}

async fn spawn_sources(sources: Vec<Box<dyn TriggerSource + Send + Sync>>, tx: Sender<Event>) {
    for mut source in sources {
        // TODO take join handle ?
        let source_tx = tx.clone();
        tokio::spawn(async move { source.run(source_tx).await });
    }
}

async fn execute_event(event: Event, context: Arc<BotContext>) {
    for action in context.actions.get(&event.name).cloned().iter().flatten() {
        let Some(handler) = context.handlers.get(&action.handler) else {
            tracing::error!("Handler not found: {}", action.handler);
            continue;
        };

        let result = match (&event.value, action.accepted_input) {
            (Ok(Value::Null), AcceptedInput::Null) => {
                Some(handler.execute(&Value::Null, &action.arguments).await)
            }
            (Ok(v), AcceptedInput::NotNull) if v != &Value::Null => {
                Some(handler.execute(v, &action.arguments).await)
            }
            (Ok(v), AcceptedInput::Ok) => Some(handler.execute(v, &action.arguments).await),
            (Err(e), AcceptedInput::Err) => Some(
                handler
                    .execute(&Value::String(format!("{e}")), &action.arguments)
                    .await,
            ),
            _ => None,
        };
        if let Some(result) = result {
            context
                .tx
                .send(Event::new(action.emit.to_string(), result))
                .await
                .unwrap();
        }
    }
}
