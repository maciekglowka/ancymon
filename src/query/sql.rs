use async_trait::async_trait;
use serde::Deserialize;
use sqlx::{any::install_default_drivers, AnyConnection, Connection, Row};

use crate::{errors::AncymonError, query::QuerySource};

#[derive(Debug, Default, Deserialize)]
struct SqlConfig {
    #[serde(rename = "connection-string")]
    connection_string: String,
}

#[derive(Default)]
pub struct SqlQuery {
    config: SqlConfig,
}
#[async_trait]
impl QuerySource for SqlQuery {
    async fn init(&mut self, config: &toml::Table) -> Result<(), AncymonError> {
        install_default_drivers();
        self.config = config
            .clone()
            .try_into()
            .map_err(|_| AncymonError::ConfigError)?;
        Ok(())
    }
    async fn execute(&self, arguments: &toml::Value) -> Result<String, AncymonError> {
        let query = arguments.as_str().ok_or(AncymonError::QuerySourceError)?;
        let mut connection = AnyConnection::connect(&self.config.connection_string)
            .await
            .unwrap();
        // let query = "SELECT timestamp, sensor_id, cast(value as real) as value FROM
        // sensors ORDER BY timestamp DESC limit ?"; sqlx::query(&query)
        //     .bind(5)
        //     .fetch_all(&connection)
        //     .await
        //     .unwrap();
        // connection
        let rows = sqlx::query(query).fetch_all(&mut connection).await.unwrap();
        for row in rows {
            println!(
                "{:?} {:?} {:?} {:?}",
                row.get::<i32, _>(0),
                row.get::<String, _>(1),
                row.get::<f64, _>(2),
                row.get::<i32, _>(3)
            );
        }
        // let row = sqlx::query(query).fetch_one(&mut connection).await.unwrap();
        // println!("{:?}", row.get::<i32, _>(0));
        Ok(String::new())
    }
}
