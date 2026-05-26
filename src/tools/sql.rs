use async_trait::async_trait;
use serde::Deserialize;
use sqlx::{
    any::{install_default_drivers, AnyArguments, AnyRow, AnyStatement, AnyTypeInfoKind},
    query::Query,
    Any, AnyConnection, Column, Connection, Executor, Row, Statement,
};

use crate::{
    errors::{AncymonError, BuildError, RuntimeError},
    tools::Tool,
    values::Value,
};

#[derive(Clone, Debug, Default, Deserialize)]
struct SqlConfig {
    #[serde(rename = "connection-string")]
    connection_string: String,
}

/// A tool for executing SQL queries against various database backends.
///
/// Configuration:
///
/// The tool is configured via a [SqlConfig] struct containing:
/// - `connection-string`: A URI-style connection string for the database.
///    (please refer to sqlx::AnyConnection docs:
///    <https://docs.rs/sqlx/latest/sqlx/struct.AnyConnection.html>)
///
/// Query Parameters:
///
/// Queries handle parameter binding via the `event` argument:
/// - Single parameter: Pass a single [Value] as the event
/// - Multiple parameters: Pass an [Value::Array] as the event
/// - Parameters are positionally bound using `?` placeholders in the SQL query
///
/// Fetch Mode:
///
/// The tool supports two fetch modes controlled by the `fetch-many`
/// argument:
/// - `false` (default): Executes query and returns first row as [Value::Array]
///   or scalar [Value] if query returns single column.
/// - `true`: Executes query and returns all rows as [Value::Array] (array of
///   arrays or array of scalars - depending on the number of columns).
///
/// Supported Data Types:
///
/// The tool supports the following SQL data types, mapped to internal
/// [Value] types:
/// - NULL -> `Value::Null`
/// - BOOLEAN -> `Value::Bool(true|false)`
/// - INTEGER (SMALLINT, INTEGER, BIGINT) -> `Value::Integer`
/// - REAL/D_FLOAT -> `Value::Float`
/// - TEXT -> `Value::String`
/// - BLOB -> Not supported (returns error)
///
/// Usage Example:
///
/// ```toml
/// [handlers.sensor-sql]
/// type = "sql"
/// connection-string = "sqlite://examples/temperature.db"
///
/// [[actions]]
/// handler = "sensor-sql"
/// event = "temperature-trigger"
/// emit = "temperature-query"
/// arguments.query = """
///   SELECT cast(value as real)
///   FROM sensors
///   WHERE sensor_id = 'temp_0'
///   ORDER BY timestamp DESC
///   LIMIT 1
/// """
/// ```
#[derive(Clone, Default)]
pub struct SqlQuery {
    config: SqlConfig,
}
impl SqlQuery {
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
    fn get_query<'a>(
        &self,
        statement: &'a AnyStatement,
        parameters: &'a Value,
    ) -> Result<Query<'a, Any, AnyArguments<'a>>, AncymonError> {
        let mut query = statement.query();
        let Some(stmt_params) = statement.parameters() else {
            return Ok(query);
        };

        let (bind_count, _) = match stmt_params {
            sqlx::Either::Left(a) => (a.len(), Some(a)),
            sqlx::Either::Right(i) => (i, None),
        };

        // Parameters should be guaranteed to be an array at this point.
        for param in parameters.as_array().unwrap() {
            query = bind_value(query, param)?;
        }
        Ok(query)
    }
}
#[async_trait]
impl Tool for SqlQuery {
    async fn init(&mut self, config: &Value) -> Result<(), AncymonError> {
        install_default_drivers();
        self.config = config
            .clone()
            .try_into()
            .map_err(|e| BuildError::Handler(format!("{e}")))?;
        Ok(())
    }
    async fn execute(&self, arguments: &Value) -> Result<Value, AncymonError> {
        let arguments: SqlArguments = arguments.try_into()?;

        let mut connection = AnyConnection::connect(&self.config.connection_string)
            .await
            .map_err(|e| RuntimeError::Handler(format!("Sql connection failed:{e}")))?;

        let stmt = connection
            .prepare(&arguments.query)
            .await
            .map_err(|e| RuntimeError::Handler(format!("Invalid sql statement: {e}")))?;

        let params = match arguments.params {
            Value::Null => Value::Array(vec![]),
            a @ Value::Array(_) => a,
            _ => {
                return Err(RuntimeError::InvalidArgumentType(
                    "Params: required array or null".to_string(),
                )
                .into())
            }
        };

        let query = self.get_query(&stmt, &params)?;

        let rows = self.fetch_many(&mut connection, query).await?;
        Ok(Value::Array(
            rows.iter()
                .map(map_row)
                .collect::<Result<Vec<_>, AncymonError>>()?,
        ))
    }
}

#[derive(Deserialize)]
struct SqlArguments {
    query: String,
    #[serde(default)]
    params: Value,
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

fn bind_value<'a>(
    query: Query<'a, Any, AnyArguments<'a>>,
    value: &'a Value,
) -> Result<Query<'a, Any, AnyArguments<'a>>, AncymonError> {
    match value {
        Value::Integer(i) => Ok(query.bind(i)),
        Value::Float(f) => Ok(query.bind(f)),
        Value::String(s) => Ok(query.bind(s)),
        _ => Err(RuntimeError::Handler(format!("Unsupported sql bind for {value:?}")).into()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    async fn db(name: &str) -> (AnyConnection, SqlQuery) {
        let connection_str = format!("sqlite:file:{name}?mode=memory&cache=shared");

        let config = Value::Map(HashMap::from_iter(vec![(
            "connection-string".to_string(),
            Value::String(connection_str.clone()),
        )]));

        let mut handler = SqlQuery::default();
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
            .execute(&Value::Map(HashMap::from_iter(vec![(
                "query".to_string(),
                Value::String("SELECT id, value FROM sensor ORDER BY value DESC;".to_string()),
            )])))
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
            .execute(&Value::Map(HashMap::from_iter(vec![(
                "query".to_string(),
                Value::String("SELECT value FROM sensor ORDER BY value;".to_string()),
            )])))
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
            .execute(&Value::Map(HashMap::from_iter(vec![(
                "query".to_string(),
                Value::String("SELECT id, value FROM sensor ORDER BY value DESC;".to_string()),
            )])))
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
            .execute(&Value::Map(HashMap::from_iter(vec![(
                "query".to_string(),
                Value::String("SELECT value FROM sensor ORDER BY value DESC;".to_string()),
            )])))
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
            .execute(&Value::Map(HashMap::from_iter(vec![(
                "query".to_string(),
                Value::String("SELECT id, ts, value, extra FROM sensor;".to_string()),
            )])))
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

    #[tokio::test]
    async fn fetch_bind_single() {
        let (mut conn, handler) = db("bind_single").await;
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
            .execute(&Value::Map(HashMap::from_iter(vec![
                (
                    "query".to_string(),
                    Value::String(
                        "SELECT value FROM sensor WHERE value > ? ORDER BY value DESC;".to_string(),
                    ),
                ),
                ("params".to_string(), Value::Array(vec![Value::Integer(4)])),
            ])))
            .await
            .unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(2, arr.len());
        assert_eq!(arr[0], Value::Integer(7));
        assert_eq!(arr[1], Value::Integer(5));
    }

    #[tokio::test]
    async fn fetch_bind_many() {
        let (mut conn, handler) = db("bind_many").await;
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
        sqlx::query("INSERT INTO sensor(id, value) VALUES ('hum', 9)")
            .execute(&mut conn)
            .await
            .unwrap();

        let result = handler
            .execute(&Value::Map(HashMap::from_iter(vec![
                (
                    "query".to_string(),
                    Value::String(
                        "SELECT value FROM sensor WHERE value > ? AND id = ? ORDER BY value DESC;"
                            .to_string(),
                    ),
                ),
                (
                    "params".to_string(),
                    Value::Array(vec![Value::Integer(4), Value::String("temp".to_string())]),
                ),
            ])))
            .await
            .unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(2, arr.len());
        assert_eq!(arr[0], Value::Integer(7));
        assert_eq!(arr[1], Value::Integer(5));
    }
}
