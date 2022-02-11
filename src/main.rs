use anyhow::Result;

mod backend;
mod blob;
mod chunker;
mod commands;
mod crypto;
mod id;
mod index;
mod repo;

fn main() -> Result<()> {
    commands::execute()
}
