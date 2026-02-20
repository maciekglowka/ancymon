use std::sync::Arc;

use ancymon::shared::discord::get_http;
use serenity::all::ClientBuilder;

#[tokio::main]
async fn main() {
    //
    let http = get_http().unwrap();
    let http = Arc::new(http);

    // let client = ClientBuilder::new_with_http(http.clone(), intents);
}
