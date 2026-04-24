//! This is the base binary for the provided Docker package.
//! It contains by default all-the built in handlers and triggers registered.

use ancymon::{
    errors::{AncymonError, ConfigError},
    handlers::{discord::DiscordDmBuilder, ical::ICalHandler, sql::SqlHandler, DebugHandler},
    triggers::{cron::CronTrigger, discord::DiscordCommandTrigger, StartupTrigger},
    Bot, Config,
};
use std::fs;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_timer(tracing_subscriber::fmt::time::ChronoLocal::rfc_3339())
        .init();

    let config = match get_config() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("{e}");
            return;
        }
    };

    Bot::default()
        .with_source_type("cron", CronTrigger::default())
        .with_source_type("discord-command", DiscordCommandTrigger::default())
        .with_source_type("startup", StartupTrigger::default())
        .with_handler_type("debug", DebugHandler)
        .with_handler_type("discord-dm", DiscordDmBuilder)
        .with_handler_type("ical", ICalHandler::default())
        .with_handler_type("sql", SqlHandler::default())
        .run(config)
        .await
        .unwrap();
}

fn get_config() -> Result<Config, AncymonError> {
    let config_str = fs::read_to_string("/srv/config.toml")
        .map_err(|e| ConfigError::ReadError(e.to_string()))?;
    Config::new(&config_str)
}
