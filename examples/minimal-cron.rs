use ancymon::{handlers::DebugHandler, triggers::cron::CronTrigger, Bot, Config};
use std::fs;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let config_str = fs::read_to_string("examples/minimal-cron.lua").unwrap();
    let config = Config::new(&config_str).unwrap();

    Bot::default()
        .with_handler_type("debug", DebugHandler)
        .with_source_type("cron", CronTrigger::default())
        .run(config)
        .await
        .unwrap();
}
