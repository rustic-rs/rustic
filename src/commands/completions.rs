use std::io::Write;

use clap::{CommandFactory, Parser, ValueEnum};
use clap_complete::{generate, shells, Generator};

#[derive(Parser)]
pub(super) struct Opts {
    #[clap(value_enum)]
    sh: Variant,
}

#[derive(Clone, ValueEnum)]
pub(super) enum Variant {
    Bash,
    Fish,
    Zsh,
}

pub(super) fn execute(opts: Opts) {
    match opts.sh {
        Variant::Bash => generate_completion(shells::Bash, &mut std::io::stdout()),
        Variant::Fish => generate_completion(shells::Fish, &mut std::io::stdout()),
        Variant::Zsh => generate_completion(shells::Zsh, &mut std::io::stdout()),
    }
}

fn generate_completion<G: Generator>(shell: G, buf: &mut dyn Write) {
    let mut command = super::Opts::command();
    generate(shell, &mut command, env!("CARGO_BIN_NAME"), buf);
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
