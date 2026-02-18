use serenity::all::Http;

use crate::errors::{AncymonError, ConfigError};

const TOKEN_VAR: &str = "DISCORD_TOKEN";

/// Get new Serenity Http intstance.
///
/// Requires bot token in DISCORD_TOKEN env variable
pub fn get_http() -> Result<Http, AncymonError> {
    let token = std::env::var(TOKEN_VAR)
        .map_err(|_| ConfigError::MissingValue(format!("Env variable not defined: {TOKEN_VAR}")))?;
    Ok(Http::new(&token))
}
