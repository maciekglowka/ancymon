use ancymon::{handlers::DebugHandler, triggers::discord::DiscordCommandTrigger, Bot, Config};
use std::fs;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let config_str = fs::read_to_string("examples/discord-command-trigger.toml").unwrap();
    let config = Config::new(&config_str).unwrap();

    Bot::default()
        .with_handler_type("debug", DebugHandler)
        .with_source_type("discord-command", DiscordCommandTrigger::default())
        .run(config)
        .await
        .unwrap();
}
