#![allow(dead_code)]

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use xtask::InstallationKind;

#[derive(Parser)]
struct Xtask {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// generate code coverage report
    Coverage {
        /// generate html report
        #[arg(long)]
        html: bool,

        /// run on complete workspace
        #[arg(short, long)]
        workspace: bool,

        /// custom Cargo target directory
        #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
        target_dir: Option<PathBuf>,
    },
    /// install dependencies
    #[command(subcommand)]
    InstallDeps(InstallationKind),
    /// show longest times taken in release build using cargo-bloat
    BloatTime {
        #[arg(short = 'p', long, help = "package to build", required = true)]
        package: Option<String>,
    },
    /// show biggest crates in release build using cargo-bloat
    BloatDeps {
        #[arg(short = 'p', long, help = "package to build", required = true)]
        package: Option<String>,
    },
    /// show longest times taken in release build using cargo
    Timings {
        /// output timings as json
        #[arg(short, long)]
        json: bool,
    },
}

fn main() -> Result<(), anyhow::Error> {
    let cli = Xtask::parse();

    match cli.command {
        Some(Commands::Coverage {
            html,
            workspace,
            target_dir,
        }) => xtask::tasks::coverage(html, workspace, target_dir)?,
        Some(Commands::InstallDeps(kind)) => xtask::tasks::install_deps(kind)?,
        Some(Commands::BloatTime { package }) => xtask::tasks::bloat_time(package)?,
        Some(Commands::BloatDeps { package }) => xtask::tasks::bloat_deps(package)?,
        Some(Commands::Timings { json }) => xtask::tasks::timings(json)?,
        None => {}
    }

    Ok(())
}
