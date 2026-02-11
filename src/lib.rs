pub mod bot;
pub mod channel;
mod config;
pub mod errors;
mod event;
pub mod query;

pub use bot::{Bot, BotBuilder};
pub use config::Config;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}
