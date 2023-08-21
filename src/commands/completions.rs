//! `completions` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use abscissa_core::{Command, Runnable};

use std::io::Write;

use clap::CommandFactory;

use clap_complete::{generate, shells, Generator};

/// `completions` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct CompletionsCmd {
    /// Shell to generate completions for
    #[clap(value_enum)]
    sh: Variant,
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub(super) enum Variant {
    Bash,
    Fish,
    Zsh,
    Powershell,
}

impl Runnable for CompletionsCmd {
    fn run(&self) {
        match self.sh {
            Variant::Bash => generate_completion(shells::Bash, &mut std::io::stdout()),
            Variant::Fish => generate_completion(shells::Fish, &mut std::io::stdout()),
            Variant::Zsh => generate_completion(shells::Zsh, &mut std::io::stdout()),
            Variant::Powershell => generate_completion(shells::PowerShell, &mut std::io::stdout()),
        }
    }
}

pub fn generate_completion<G: Generator>(shell: G, buf: &mut dyn Write) {
    let mut command = crate::commands::EntryPoint::command();
    generate(
        shell,
        &mut command,
        option_env!("CARGO_BIN_NAME").unwrap_or("rustic"),
        buf,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completions() {
        generate_completion(shells::Bash, &mut std::io::sink());
        generate_completion(shells::Fish, &mut std::io::sink());
        generate_completion(shells::Zsh, &mut std::io::sink());
    }
}
