use async_trait::async_trait;
use serde::Deserialize;
use sqlx::{
    any::{install_default_drivers, AnyArguments, AnyRow, AnyTypeInfoKind},
    query::Query,
    Any, AnyConnection, Column, Connection, Row, TypeInfo,
};

use crate::{
    errors::{AncymonError, BuildError, ConfigError, RuntimeError},
    events::EventValue,
    handlers::{EventHandler, HandlerBuilder},
    values::Value,
};

#[derive(Debug, Default, Deserialize)]
struct SqlConfig {
    #[serde(rename = "connection-string")]
    connection_string: String,
}

#[derive(Default)]
pub struct SqlHandler {
    config: SqlConfig,
}
impl SqlHandler {
    async fn fetch_one<'a>(
        &self,
        connection: &mut AnyConnection,
        query: Query<'a, Any, AnyArguments<'a>>,
    ) -> Result<(), AncymonError> {
        let row = query
            .fetch_one(connection)
            .await
            .map_err(|e| RuntimeError::Handler(format!("Sql row fetch failed {e}")))?;
        Ok(())
    }
    async fn fetch_many<'a>(
        &self,
        connection: &mut AnyConnection,
        query: Query<'a, Any, AnyArguments<'a>>,
    ) {
        query.fetch_all(connection).await;
    }
}
#[async_trait]
impl EventHandler for SqlHandler {
    async fn init(&mut self, config: &toml::Table) -> Result<(), AncymonError> {
        install_default_drivers();
        self.config = config
            .clone()
            .try_into()
            .map_err(|e| BuildError::Handler(format!("{e}")))?;
        Ok(())
    }
    async fn execute(&self, event: &Value, arguments: &Value) -> EventValue {
        let arguments: SqlArguments = arguments.clone().try_into()?;

        let mut connection = AnyConnection::connect(&self.config.connection_string)
            .await
            .map_err(|e| RuntimeError::Handler(format!("Sql connection failed:{e}")))?;

        // let query = sqlx::query(&arguments.query).bind(2);

        // if arguments.fetch_many {
        //     self.fetch_many(&mut connection, query).await;
        // } else {
        //     self.fetch_one(&mut connection, query).await;
        // }

        // let a = query.fetch_one(&mut connection);
        // let rows = if arguments.fetch_many {
        //     query.fetch_all(&mut connection)
        // } else {
        //     query.fetch_one(&mut connection)
        // };

        // .unwrap();

        // for row in rows {
        //     println!(
        //         "{:?} {:?} {:?} {:?}",
        //         row.get::<i32, _>(0),
        //         row.get::<String, _>(1),
        //         row.get::<f64, _>(2),
        //         row.get::<i32, _>(3)
        //     );
        // }

        // let row = sqlx::query(query).fetch_one(&mut connection).await.unwrap();
        // println!("{:?}", row.get::<i32, _>(0));
        Ok(Value::Integer(0))
    }
}

pub struct SqlBuilder;
impl HandlerBuilder for SqlBuilder {
    fn build(&self) -> Result<Box<dyn EventHandler + Send + Sync>, AncymonError> {
        Ok(Box::new(SqlHandler::default()))
    }
}

struct SqlArguments {
    query: String,
    fetch_many: bool,
    bind_input: bool,
}
impl TryFrom<Value> for SqlArguments {
    type Error = AncymonError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let map = value
            .as_map()
            .ok_or(ConfigError::InvalidValueType("Expected map".to_string()))?;

        let query = map
            .get("query")
            .ok_or(ConfigError::MissingValue(
                "Field `query` is required".to_string(),
            ))?
            .as_str()
            .ok_or(ConfigError::InvalidValueType("Expected string".to_string()))?
            .to_string();

        let fetch_many = if let Some(v) = map.get("fetch-many") {
            v.as_bool()
                .ok_or(ConfigError::InvalidValueType("Expected bool".to_string()))?
        } else {
            false
        };

        let bind_input = if let Some(v) = map.get("bind-input") {
            v.as_bool()
                .ok_or(ConfigError::InvalidValueType("Expected bool".to_string()))?
        } else {
            false
        };

        Ok(Self {
            query,
            fetch_many,
            bind_input,
        })
    }
}

fn map_row(row: AnyRow) {
    if row.is_empty() {}
    if row.len() == 1 {
        let kind = row.columns().get(0).unwrap().type_info().kind();
        //     .        let v = row.get(0);
    }
}

fn map_db_value(row: AnyRow, idx: usize) -> Result<Value, AncymonError> {
    let kind = row
        .columns()
        .get(idx)
        .ok_or(RuntimeError::Handler(format!(
            "Column at index {idx} not found"
        )))?
        .type_info()
        .kind();
    match kind {
        AnyTypeInfoKind::Null => Ok(Value::Null),
        AnyTypeInfoKind::Bool => Ok(Value::Bool(row.try_get::<bool, _>(idx).map_err(|e| {
            RuntimeError::Handler(format!("Expected bool at column index {idx}. {e}"))
        })?)),
        AnyTypeInfoKind::SmallInt | AnyTypeInfoKind::Integer | AnyTypeInfoKind::BigInt => {
            Ok(Value::Integer(row.try_get::<i64, _>(idx).map_err(|e| {
                RuntimeError::Handler(format!("Expected integer at column index {idx}. {e}"))
            })?))
        }
        AnyTypeInfoKind::Real | AnyTypeInfoKind::Double => {
            Ok(Value::Float(row.try_get::<f64, _>(idx).map_err(|e| {
                RuntimeError::Handler(format!("Expected float at column index {idx}. {e}"))
            })?))
        }
        AnyTypeInfoKind::Text => Ok(Value::String(row.try_get::<String, _>(idx).map_err(
            |e| RuntimeError::Handler(format!("Expected string at column index {idx}. {e}")),
        )?)),
        AnyTypeInfoKind::Blob => {
            Err(RuntimeError::InvalidArgumentType("Blobs are not supported".to_string()).into())
        }
    }
}
