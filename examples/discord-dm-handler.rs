use ancymon::{handlers::discord::DiscordDmBuilder, triggers::StartupTrigger, Bot, Config};
use std::fs;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let config_str = fs::read_to_string("examples/discord-dm-handler.toml").unwrap();
    let config = Config::new(&config_str).unwrap();

    Bot::default()
        .with_handler_type("discord-dm", DiscordDmBuilder)
        .with_source_type("startup", StartupTrigger::default())
        .run(config)
        .await
        .unwrap();
}
