mod app;
mod cli;
mod commands;
mod dotenv;
mod sync;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}
