use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use tokio::sync::mpsc::{Receiver, Sender};

use crate::{
    actions::{AcceptedInput, Action},
    config::Config,
    errors::{AncymonError, ConfigError},
    events::{pack_error, Event},
    tools::{Tool, ToolBuilder},
    triggers::{Trigger, TriggerSource},
    values::Value,
};

const QUEUE_SIZE: usize = 256;

struct BotContext {
    actions: HashMap<String, Vec<Action>>,
    tools: HashMap<String, Box<dyn Tool + Send + Sync>>,
    tx: Sender<Event>,
}

#[derive(Default)]
pub struct Bot {
    tool_builders: HashMap<String, Box<dyn ToolBuilder>>,
    trigger_sources: HashMap<String, Box<dyn TriggerSource + Send + Sync>>,
}
impl Bot {
    pub async fn run(mut self, config: Config) -> Result<(), AncymonError> {
        let tools = self.build_tools(&config).await?;
        let actions = self.build_actions(&config).await?;

        let active_sources = self.init_trigger_sources(&config).await?;
        let sources = self
            .trigger_sources
            .into_iter()
            .filter(|(k, _)| active_sources.contains(k))
            .map(|(_, v)| v)
            .collect();

        let (tx, rx) = tokio::sync::mpsc::channel(QUEUE_SIZE);

        let context = BotContext {
            actions,
            tools,
            tx: tx.clone(),
        };

        spawn_sources(sources, tx).await;
        run(context, rx).await?;

        Ok(())
    }

    pub fn with_tool_type<T: ToolBuilder + 'static>(
        mut self,
        name: impl Into<String>,
        builder: T,
    ) -> Self {
        self.tool_builders
            .insert(name.into(), Box::new(builder) as Box<dyn ToolBuilder>);
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

    async fn build_tool(
        &self,
        config: &Config,
    ) -> Result<HashMap<String, Box<dyn Tool + Send + Sync>>, AncymonError> {
        let mut tools = HashMap::new();

        for (name, tool_config) in config.tools.iter() {
            let builder = tool_config
                .as_map()
                .ok_or(ConfigError::InvalidValueType(format!(
                    "Expected map as tool config. Found: {tool_config:?}"
                )))?
                .get("type")
                .ok_or(ConfigError::MissingValue(format!(
                    "Key not found: `type` at tool config {name}"
                )))?
                .as_str()
                .ok_or(ConfigError::InvalidValueType(format!(
                    "Expected string for key `type` at tool config {name}"
                )))?;
            let mut tool = self
                .tool_builders
                .get(builder)
                .ok_or(ConfigError::InvalidToolType(builder.to_string()))?
                .build()?;
            tool.init(tool_config).await?;
            tools.insert(name.to_string(), tool);
        }
        Ok(tools)
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

    /// Returns source list to activate
    async fn init_trigger_sources(
        &mut self,
        config: &Config,
    ) -> Result<HashSet<String>, AncymonError> {
        let mut triggers: HashMap<String, Vec<Trigger>> = HashMap::new();
        let mut sources = HashSet::new();

        for trigger in config.triggers.iter() {
            if let Some(entry) = triggers.get_mut(&trigger.source) {
                entry.push(trigger.clone());
                continue;
            }
            triggers.insert(trigger.source.to_string(), vec![trigger.clone()]);
        }

        for (source_name, triggers) in triggers {
            sources.insert(source_name.to_string());
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

        Ok(sources)
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
    let Some(actions) = context.actions.get(&event.name) else {
        return;
    };
    let entries = actions
        .iter()
        .enumerate()
        .flat_map(|(i, action)| match (&event.value, action.accepted_input) {
            (Ok(Value::Null), AcceptedInput::Null) => Some(i),
            (Ok(v), AcceptedInput::NotNull) if v != &Value::Null => Some(i),
            (Ok(_), AcceptedInput::Ok) => Some(i),
            (Err(_), AcceptedInput::Err) => Some(i),
            _ => None,
        })
        .collect::<Vec<_>>();

    if entries.len() == 1 {
        // Fast track no spawn.
        execute_single(event, entries[0], &context).await;
        return;
    }

    for action_idx in entries {
        let context = Arc::clone(&context);
        let event = event.clone();
        tokio::spawn(async move { execute_single(event, action_idx, &context).await });
    }
}

async fn execute_single(mut event: Event, action_idx: usize, context: &Arc<BotContext>) {
    let Some(actions) = context.actions.get(&event.name) else {
        // Should never actually happen.
        return;
    };
    let action = &actions[action_idx];
    let Some(tool) = context.tools.get(&action.tool) else {
        // Should never actually happen.
        tracing::error!("Tool not found: {}", action.handler);
        return;
    };

    let value = match &event.value {
        Ok(v) => v,
        Err(e) => e,
    };
    let mut retries = action.max_retries;

    loop {
        let result = match tool
            .execute(value, &action.arguments, &mut event.meta)
            .await
        {
            Ok(a) => Ok(a),
            Err(e) => {
                if retries > 0 {
                    tracing::error!(
                        "Action execution failed: {e}. Retrying in {}s",
                        action.retry_delay
                    );
                } else {
                    tracing::error!("Action execution failed: {e}.",);
                }
                Err(pack_error(value.clone(), e))
            }
        };

        let retry = result.is_err() && retries > 0;
        context
            .tx
            .send(Event::with_meta(
                action.emit.to_string(),
                result,
                event.meta.clone(),
            ))
            .await
            .unwrap();

        if retry {
            retries -= 1;
            tokio::time::sleep(Duration::from_secs(action.retry_delay)).await;
            continue;
        }
        break;
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::triggers::StartupTrigger;

    use super::*;

    struct Counter(Arc<AtomicU64>);
    #[async_trait]
    impl Tool for Counter {
        async fn execute(&self, arguments: &Value) -> Result<Value, AncymonError> {
            let a = arguments.as_int().ok_or(ConfigError::InvalidValue(format!(
                "Invalid arguments {arguments:?}"
            )))? as u64;
            let v = self.0.fetch_add(a, Ordering::Relaxed);
            Ok(Value::Integer(v as i64))
        }
    }
    struct CounterBuilder(Arc<AtomicU64>);
    impl ToolBuilder for CounterBuilder {
        fn build(&self) -> Result<Box<dyn Tool + Send + Sync>, AncymonError> {
            Ok(Box::new(Counter(self.0.clone())))
        }
    }

    async fn assert_atomic(var: &Arc<AtomicU64>, expected: u64, timeout: u64) {
        tokio::time::timeout(Duration::from_millis(timeout), async move {
            while var.load(Ordering::Relaxed) != expected {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn trigger() {
        let config = r#"
          [sources.startup]  
          type = "startup"

          [[triggers]]
          source = "startup"
          emit = "start"

          [handlers.counter]
          type = "counter"

          [[actions]]
          handler = "counter"
          event = "start"
          emit = ""
          arguments = 13
        "#;

        let local = tokio::task::LocalSet::new();
        local
            .run_until(async move {
                let count = Arc::new(AtomicU64::new(0));
                let spawned_count = count.clone();

                let config = Config::new(config).unwrap();

                let bot = tokio::task::spawn_local(async move {
                    Bot::default()
                        .with_source_type("startup", StartupTrigger::default())
                        .with_handler_type("counter", CounterBuilder(spawned_count))
                        .run(config)
                        .await
                        .unwrap()
                });
                assert_atomic(&count, 13, 100).await;
                bot.abort();
            })
            .await;
    }

    #[tokio::test]
    async fn trigger_chained() {
        let config = r#"
          [sources.startup]  
          type = "startup"

          [[triggers]]
          source = "startup"
          emit = "start"

          [handlers.counter]
          type = "counter"

          [[actions]]
          handler = "counter"
          event = "start"
          emit = "middle"
          arguments = 13

          [[actions]]
          handler = "counter"
          event = "middle"
          emit = "end"
          arguments = 17
        "#;

        let local = tokio::task::LocalSet::new();
        local
            .run_until(async move {
                let count = Arc::new(AtomicU64::new(0));
                let spawned_count = count.clone();

                let config = Config::new(config).unwrap();

                let bot = tokio::task::spawn_local(async move {
                    Bot::default()
                        .with_source_type("startup", StartupTrigger::default())
                        .with_handler_type("counter", CounterBuilder(spawned_count))
                        .run(config)
                        .await
                        .unwrap()
                });
                assert_atomic(&count, 30, 100).await;
                bot.abort();
            })
            .await;
    }

    #[tokio::test]
    async fn trigger_parallel() {
        let config = r#"
          [sources.startup]  
          type = "startup"

          [[triggers]]
          source = "startup"
          emit = "start"

          [handlers.counter]
          type = "counter"

          [[actions]]
          handler = "counter"
          event = "start"
          emit = "end-1"
          arguments = 13

          [[actions]]
          handler = "counter"
          event = "start"
          emit = "end-2"
          arguments = 17
        "#;

        let local = tokio::task::LocalSet::new();
        local
            .run_until(async move {
                let count = Arc::new(AtomicU64::new(0));
                let spawned_count = count.clone();

                let config = Config::new(config).unwrap();

                let bot = tokio::task::spawn_local(async move {
                    Bot::default()
                        .with_source_type("startup", StartupTrigger::default())
                        .with_handler_type("counter", CounterBuilder(spawned_count))
                        .run(config)
                        .await
                        .unwrap()
                });
                assert_atomic(&count, 30, 100).await;
                bot.abort();
            })
            .await;
    }

    #[tokio::test]
    async fn retry_err() {
        let config = r#"
          [sources.startup]  
          type = "startup"

          [[triggers]]
          source = "startup"
          emit = "start"

          [handlers.counter]
          type = "counter"

          [[actions]]
          handler = "counter"
          event = "start"
          emit = "middle"
          arguments = "13"
          max-retries = 5

          [[actions]]
          handler = "counter"
          event = "middle"
          emit = "end"
          accepted-input = "Err"
          arguments = 17
        "#;

        let local = tokio::task::LocalSet::new();
        local
            .run_until(async move {
                let count = Arc::new(AtomicU64::new(0));
                let spawned_count = count.clone();

                let config = Config::new(config).unwrap();

                let bot = tokio::task::spawn_local(async move {
                    Bot::default()
                        .with_source_type("startup", StartupTrigger::default())
                        .with_handler_type("counter", CounterBuilder(spawned_count))
                        .run(config)
                        .await
                        .unwrap()
                });
                assert_atomic(&count, (5 + 1) * 17, 100).await;
                bot.abort();
            })
            .await;
    }

    #[tokio::test]
    async fn retry_err_parallel() {
        let config = r#"
          [sources.startup]  
          type = "startup"

          [[triggers]]
          source = "startup"
          emit = "start"

          [handlers.counter]
          type = "counter"

          [[actions]]
          handler = "counter"
          event = "start"
          emit = "middle-1"
          arguments = "13"
          max-retries = 5

          [[actions]]
          handler = "counter"
          event = "start"
          emit = "middle-2"
          arguments = "13"
          max-retries = 3

          [[actions]]
          handler = "counter"
          event = "middle-1"
          emit = "end"
          accepted-input = "Err"
          arguments = 17

          [[actions]]
          handler = "counter"
          event = "middle-2"
          emit = "end"
          accepted-input = "Err"
          arguments = 19
        "#;

        let local = tokio::task::LocalSet::new();
        local
            .run_until(async move {
                let count = Arc::new(AtomicU64::new(0));
                let spawned_count = count.clone();

                let config = Config::new(config).unwrap();

                let bot = tokio::task::spawn_local(async move {
                    Bot::default()
                        .with_source_type("startup", StartupTrigger::default())
                        .with_handler_type("counter", CounterBuilder(spawned_count))
                        .run(config)
                        .await
                        .unwrap()
                });
                assert_atomic(&count, (5 + 1) * 17 + (3 + 1) * 19, 100).await;
                bot.abort();
            })
            .await;
    }

    #[tokio::test]
    async fn retry_err_mixed() {
        let config = r#"
          [sources.startup]  
          type = "startup"

          [[triggers]]
          source = "startup"
          emit = "start"

          [handlers.counter]
          type = "counter"

          [[actions]]
          handler = "counter"
          event = "start"
          emit = "middle-1"
          arguments = 13
          max-retries = 5

          [[actions]]
          handler = "counter"
          event = "start"
          emit = "middle-2"
          arguments = "13"
          max-retries = 3

          [[actions]]
          handler = "counter"
          event = "middle-1"
          emit = "end"
          accepted-input = "Err"
          arguments = 17

          [[actions]]
          handler = "counter"
          event = "middle-2"
          emit = "end"
          accepted-input = "Err"
          arguments = 19
        "#;

        let local = tokio::task::LocalSet::new();
        local
            .run_until(async move {
                let count = Arc::new(AtomicU64::new(0));
                let spawned_count = count.clone();

                let config = Config::new(config).unwrap();

                let bot = tokio::task::spawn_local(async move {
                    Bot::default()
                        .with_source_type("startup", StartupTrigger::default())
                        .with_handler_type("counter", CounterBuilder(spawned_count))
                        .run(config)
                        .await
                        .unwrap()
                });
                // First one succeeds immediately, second one goes through all the retries.
                assert_atomic(&count, 13 + (3 + 1) * 19, 100).await;
                bot.abort();
            })
            .await;
    }
}
