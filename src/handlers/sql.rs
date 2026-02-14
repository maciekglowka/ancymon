use async_trait::async_trait;
use serde::Deserialize;
use sqlx::{
    any::{install_default_drivers, AnyArguments, AnyRow, AnyTypeInfoKind},
    query::Query,
    Any, AnyConnection, Column, Connection, Row,
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
    ) -> Result<AnyRow, AncymonError> {
        let row = query
            .fetch_one(connection)
            .await
            .map_err(|e| RuntimeError::Handler(format!("Sql fetch one failed {e}")))?;
        Ok(row)
    }
    async fn fetch_many<'a>(
        &self,
        connection: &mut AnyConnection,
        query: Query<'a, Any, AnyArguments<'a>>,
    ) -> Result<Vec<AnyRow>, AncymonError> {
        let rows = query
            .fetch_all(connection)
            .await
            .map_err(|e| RuntimeError::Handler(format!("Sql fetch many failed {e}")))?;
        Ok(rows)
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

        let query = sqlx::query(&arguments.query).bind(2);

        if arguments.fetch_many {
            let rows = self.fetch_many(&mut connection, query).await?;
            Ok(Value::Array(
                rows.iter()
                    .map(map_row)
                    .collect::<Result<Vec<_>, AncymonError>>()?,
            ))
        } else {
            let row = self.fetch_one(&mut connection, query).await?;
            map_row(&row)
        }
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
    bind_one: bool,
    bind_many: bool,
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
        let bind_one = if let Some(v) = map.get("bind-one") {
            v.as_bool()
                .ok_or(ConfigError::InvalidValueType("Expected bool".to_string()))?
        } else {
            false
        };
        let bind_many = if let Some(v) = map.get("bind-many") {
            v.as_bool()
                .ok_or(ConfigError::InvalidValueType("Expected bool".to_string()))?
        } else {
            false
        };

        Ok(Self {
            query,
            fetch_many,
            bind_one,
            bind_many,
        })
    }
}

fn map_row(row: &AnyRow) -> Result<Value, AncymonError> {
    if row.is_empty() {
        return Ok(Value::Null);
    }
    if row.len() == 1 {
        return map_db_value(row, 0);
    }
    let v = (0..row.len())
        .map(|i| map_db_value(row, i))
        .collect::<Result<Vec<_>, AncymonError>>()?;
    Ok(Value::Array(v))
}

macro_rules! map_nullable {
    ($variant:ident, $row:ident, $ty:ty, $idx:expr) => {
        if let Some(value) = $row.try_get::<Option<$ty>, _>($idx).map_err(|e| {
            RuntimeError::Handler(format!("Invalid type for column at index {}. {}", $idx, e))
        })? {
            Value::$variant(value)
        } else {
            Value::Null
        }
    };
}

fn map_db_value(row: &AnyRow, idx: usize) -> Result<Value, AncymonError> {
    let kind = row
        .columns()
        .get(idx)
        .ok_or(RuntimeError::Handler(format!(
            "Column at index {idx} not found"
        )))?
        .type_info()
        .kind();

    println!("{kind:?}");

    match kind {
        AnyTypeInfoKind::Null => Ok(Value::Null),
        AnyTypeInfoKind::Bool => Ok(map_nullable!(Bool, row, bool, idx)),
        AnyTypeInfoKind::SmallInt | AnyTypeInfoKind::Integer | AnyTypeInfoKind::BigInt => {
            Ok(map_nullable!(Integer, row, i64, idx))
        }
        AnyTypeInfoKind::Real | AnyTypeInfoKind::Double => Ok(map_nullable!(Float, row, f64, idx)),
        AnyTypeInfoKind::Text => Ok(map_nullable!(String, row, String, idx)),
        AnyTypeInfoKind::Blob => {
            Err(RuntimeError::InvalidArgumentType("Blobs are not supported".to_string()).into())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, str::FromStr};

    use super::*;

    async fn db(name: &str) -> (AnyConnection, SqlHandler) {
        let connection_str = format!("sqlite:file:{name}?mode=memory&cache=shared");
        let config =
            toml::Table::from_str(&format!("connection-string = \"{connection_str}\"")).unwrap();

        let mut handler = SqlHandler::default();
        handler.init(&config).await.unwrap();

        let conn = AnyConnection::connect(&connection_str)
            .await
            .map_err(|e| RuntimeError::Handler(format!("Sql connection failed:{e}")))
            .unwrap();
        (conn, handler)
    }

    #[tokio::test]
    async fn fetch_one() {
        let (mut conn, handler) = db("fetch_one").await;
        sqlx::query("CREATE TABLE sensor ( id text, value integer );")
            .execute(&mut conn)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sensor(id, value) VALUES ('temp', 3)")
            .execute(&mut conn)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sensor(id, value) VALUES ('temp', 7)")
            .execute(&mut conn)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sensor(id, value) VALUES ('temp', 5)")
            .execute(&mut conn)
            .await
            .unwrap();

        let result = handler
            .execute(
                &Value::Null,
                &Value::Map(HashMap::from_iter(vec![(
                    "query".to_string(),
                    Value::String("SELECT id, value FROM sensor ORDER BY value DESC;".to_string()),
                )])),
            )
            .await
            .unwrap();
        assert_eq!(
            result,
            Value::Array(vec![Value::String("temp".to_string()), Value::Integer(7)])
        )
    }
    #[tokio::test]
    async fn fetch_one_scalar() {
        let (mut conn, handler) = db("fetch_one_scalar").await;
        sqlx::query("CREATE TABLE sensor ( id text, value integer );")
            .execute(&mut conn)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sensor(id, value) VALUES ('temp', 9)")
            .execute(&mut conn)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sensor(id, value) VALUES ('temp', 7)")
            .execute(&mut conn)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sensor(id, value) VALUES ('temp', 15)")
            .execute(&mut conn)
            .await
            .unwrap();

        let result = handler
            .execute(
                &Value::Null,
                &Value::Map(HashMap::from_iter(vec![(
                    "query".to_string(),
                    Value::String("SELECT value FROM sensor ORDER BY value;".to_string()),
                )])),
            )
            .await
            .unwrap();
        assert_eq!(result, Value::Integer(7))
    }
    #[tokio::test]
    async fn fetch_many() {
        let (mut conn, handler) = db("fetch_many").await;
        sqlx::query("CREATE TABLE sensor ( id text, value integer );")
            .execute(&mut conn)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sensor(id, value) VALUES ('temp', 3)")
            .execute(&mut conn)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sensor(id, value) VALUES ('temp', 7)")
            .execute(&mut conn)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sensor(id, value) VALUES ('temp', 5)")
            .execute(&mut conn)
            .await
            .unwrap();

        let result = handler
            .execute(
                &Value::Null,
                &Value::Map(HashMap::from_iter(vec![
                    (
                        "query".to_string(),
                        Value::String(
                            "SELECT id, value FROM sensor ORDER BY value DESC;".to_string(),
                        ),
                    ),
                    ("fetch-many".to_string(), Value::Bool(true)),
                ])),
            )
            .await
            .unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(
            arr[0],
            Value::Array(vec![Value::String("temp".to_string()), Value::Integer(7)])
        );
        assert_eq!(
            arr[1],
            Value::Array(vec![Value::String("temp".to_string()), Value::Integer(5)])
        );
        assert_eq!(
            arr[2],
            Value::Array(vec![Value::String("temp".to_string()), Value::Integer(3)])
        );
    }
    #[tokio::test]
    async fn fetch_many_scalar() {
        let (mut conn, handler) = db("fetch_many_scalar").await;
        sqlx::query("CREATE TABLE sensor ( id text, value integer );")
            .execute(&mut conn)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sensor(id, value) VALUES ('temp', 3)")
            .execute(&mut conn)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sensor(id, value) VALUES ('temp', 7)")
            .execute(&mut conn)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sensor(id, value) VALUES ('temp', 5)")
            .execute(&mut conn)
            .await
            .unwrap();

        let result = handler
            .execute(
                &Value::Null,
                &Value::Map(HashMap::from_iter(vec![
                    (
                        "query".to_string(),
                        Value::String("SELECT value FROM sensor ORDER BY value DESC;".to_string()),
                    ),
                    ("fetch-many".to_string(), Value::Bool(true)),
                ])),
            )
            .await
            .unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(arr[0], Value::Integer(7));
        assert_eq!(arr[1], Value::Integer(5));
        assert_eq!(arr[2], Value::Integer(3));
    }

    #[tokio::test]
    async fn fetch_types() {
        let (mut conn, handler) = db("fetch_types").await;
        // Apparently bool is not supported at the moment by sqlx@sqlite
        sqlx::query("CREATE TABLE sensor ( id text, ts integer, value real, extra integer );")
            .execute(&mut conn)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sensor(id, ts, value, extra) VALUES ('temp', 1771072386, 23.75, null)",
        )
        .execute(&mut conn)
        .await
        .unwrap();

        let result = handler
            .execute(
                &Value::Null,
                &Value::Map(HashMap::from_iter(vec![(
                    "query".to_string(),
                    Value::String("SELECT id, ts, value, extra FROM sensor;".to_string()),
                )])),
            )
            .await
            .unwrap();

        assert_eq!(
            result,
            Value::Array(vec![
                Value::String("temp".to_string()),
                Value::Integer(1771072386),
                Value::Float(23.75),
                Value::Null,
            ])
        )
    }
}
