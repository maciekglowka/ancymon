use std::collections::HashMap;

use crate::{config::Config, errors::AncymonError, event::Event, query::QuerySource};

pub struct Bot {
    events: Vec<Event>,
    query_sources: HashMap<String, Box<dyn QuerySource + Send>>,
}
impl Bot {
    pub async fn execute_event(&self, event: &Event) -> Result<(), AncymonError> {
        let source = self.query_sources.get(&event.query_source).unwrap();
        source.execute(&event.arguments).await.unwrap();
        Ok(())
    }
    // TEMP
    pub async fn run(&self) {
        for event in self.events.iter() {
            self.execute_event(event).await.unwrap();
        }
    }
}

#[derive(Default)]
pub struct BotBuilder {
    query_types: HashMap<String, Box<dyn Fn() -> Box<dyn QuerySource + Send>>>,
}
impl BotBuilder {
    pub async fn build(self, config: Config) -> Result<Bot, AncymonError> {
        let mut query_sources = HashMap::new();

        for (name, source_config) in config.query_sources.into_iter() {
            let t = source_config
                .get("type")
                .ok_or(AncymonError::ConfigError)?
                .as_str()
                .ok_or(AncymonError::ConfigError)?;
            let mut source = (self.query_types.get(t).ok_or(AncymonError::ConfigError)?)();
            source.init(&source_config).await?;
            query_sources.insert(name, source);
        }

        let events = config.events;

        Ok(Bot {
            query_sources,
            events,
        })
    }
    pub fn with_query_type<T: QuerySource + Send + 'static + Default>(
        mut self,
        name: impl Into<String>,
    ) -> Self {
        let get = || Box::new(T::default()) as Box<dyn QuerySource + Send>;
        self.query_types.insert(name.into(), Box::new(get));
        self
    }
}
