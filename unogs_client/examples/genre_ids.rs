use std::env;
use unogs_client::UnogsClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = UnogsClient::new(&env::var("RAPIDAPI_KEY")?)?;
    dbg!(client.genre_ids().await?);
    Ok(())
}
