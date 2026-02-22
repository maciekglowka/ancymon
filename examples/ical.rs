use ancymon::{handlers::ical::ICalHandler, triggers::StartupTrigger, Bot, Config};
use std::fs;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let config_str = fs::read_to_string("examples/ical.toml").unwrap();
    let config = Config::new(&config_str).unwrap();

    Bot::default()
        .with_handler_type("ical", ICalHandler::default())
        .with_source_type("startup", StartupTrigger::default())
        .run(config)
        .await
        .unwrap();
}
