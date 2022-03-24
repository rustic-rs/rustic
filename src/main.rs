use anyhow::Result;

mod archiver;
mod backend;
mod blob;
mod chunker;
mod commands;
mod crypto;
mod id;
mod index;
mod repo;

#[tokio::main]
async fn main() -> Result<()> {
    commands::execute().await
}
