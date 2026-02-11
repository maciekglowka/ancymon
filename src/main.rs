use async_trait::async_trait;
use serenity::{
    all::{Context, CreateMessage, EventHandler, GatewayIntents, Message, Ready, UserId},
    Client,
};
use std::fs;

use ancymon::{query::sql::SqlQuery, BotBuilder, Config};

#[tokio::main]
async fn main() {
    let token = std::env::var("DISCORD_TOKEN").unwrap();
    let intents = GatewayIntents::DIRECT_MESSAGES;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .unwrap();

    client.start().await.unwrap();
}

// #[tokio::main]
// async fn main() {
//     let config_str = fs::read_to_string("example_config.toml").unwrap();
//     let config = Config::new(&config_str).unwrap();

//     let mut bot = BotBuilder::default()
//         .with_query_type::<SqlQuery>("sql")
//         .build(config)
//         .await
//         .unwrap();
//     bot.run().await;
// }

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        println!("{msg:?}");
        msg.reply(&ctx, "Pong").await;
    }
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{ready:?}");
        let id = std::env::var("DISCORD_USER_ID")
            .unwrap()
            .parse::<u64>()
            .unwrap();

        let builder = CreateMessage::new().content("Hello from Ancymon!");
        if let Err(e) = UserId::new(id).direct_message(&ctx, builder).await {
            println!("{e}");
        }
    }
}
