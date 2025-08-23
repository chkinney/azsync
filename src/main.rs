mod app;
mod cli;
mod commands;
mod dotenv;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}
