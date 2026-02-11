use ancymon::{handlers::DebugBuilder, triggers::cron::CronTrigger, BotBuilder, Config};
use std::fs;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let config_str = fs::read_to_string("examples/minimal-cron.toml").unwrap();
    let config = Config::new(&config_str).unwrap();

    let mut bot = BotBuilder::default()
        .with_handler("debug", DebugBuilder)
        .with_source("cron", CronTrigger::default())
        .build(config)
        .await
        .unwrap();

    bot.run().await;
}
