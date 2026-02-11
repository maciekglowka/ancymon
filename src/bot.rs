use std::collections::HashMap;

use crate::{
    actions::Action,
    config::Config,
    errors::{AncymonError, ConfigError},
    events::Event,
    handlers::{EventHandler, HandlerBuilder},
    triggers::{Trigger, TriggerSource},
};

pub struct Bot {
    actions: HashMap<String, Vec<Action>>,
    handlers: HashMap<String, Box<dyn EventHandler + Send>>,
    sources: Vec<Box<dyn TriggerSource + Send>>,
}
impl Bot {
    // pub async fn execute_event(&self, event: &Event) -> Result<(), AncymonError>
    // {     let source = self.query_sources.get(&event.query_source).unwrap();
    //     source.execute(&event.arguments).await.unwrap();
    //     Ok(())
    // }
    pub async fn run(&mut self) {
        tracing::info!("Bot is starting...");
        let (tx, mut rx) = tokio::sync::mpsc::channel(256);

        self.spawn_sources(tx).await;

        while let Some(event) = rx.recv().await {
            println!("{event:?}");
        }
    }

    async fn spawn_sources(&mut self, tx: tokio::sync::mpsc::Sender<Event>) {
        for mut source in self.sources.drain(..) {
            // TODO take join handle
            let source_tx = tx.clone();
            tokio::spawn(async move { source.run(source_tx).await });
        }
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
