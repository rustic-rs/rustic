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
        Variant::Bash => generate_completion(shells::Bash),
        Variant::Fish => generate_completion(shells::Fish),
        Variant::Zsh => generate_completion(shells::Zsh),
    }
}

fn generate_completion<G: Generator>(shell: G) {
    let mut command = super::Opts::command();
    generate(
        shell,
        &mut command,
        env!("CARGO_BIN_NAME"),
        &mut std::io::stdout(),
    );
}
