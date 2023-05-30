use anyhow::Result;
use clap::Subcommand;
use duct::cmd;

#[derive(Subcommand, Debug, Clone, Copy)]
pub enum InstallationKind {
    /// only essential deps (cargo-bloat, llvm-tools-preview, grcov)
    Essentials,
    /// only code coverage deps (llvm-tools-preview, grcov)
    CodeCoverage,
    /// only mutation test deps (cargo-mutants)
    Mutants,
    /// only deps for fuzzing (cargo-fuzz)
    Fuzzing,
    /// only deps for (de-)bloating (cargo-bloat)
    Bloat,
    /// full deps (all of above)
    Full,
}

///
/// Install cargo tools
///
/// # Errors
/// Errors if one of the commands failed
///
pub fn install_deps(kind: InstallationKind) -> Result<()> {
    match kind {
        InstallationKind::Essentials => {
            install_essentials()?;
            Ok(())
        }
        InstallationKind::Full => {
            install_essentials()?;
            cmd!("cargo", "install", "cargo-watch").run()?;
            cmd!("cargo", "install", "cargo-hack").run()?;
            cmd!("cargo", "install", "cargo-mutants").run()?;
            Ok(())
        }
        InstallationKind::CodeCoverage => {
            cmd!("rustup", "component", "add", "llvm-tools-preview").run()?;
            cmd!("cargo", "install", "grcov").run()?;
            Ok(())
        }
        InstallationKind::Mutants => {
            cmd!("cargo", "install", "cargo-mutants").run()?;
            Ok(())
        }
        InstallationKind::Fuzzing => {
            cmd!("cargo", "install", "cargo-fuzz").run()?;
            Ok(())
        }
        InstallationKind::Bloat => {
            cmd!("cargo", "install", "cargo-bloat").run()?;
            Ok(())
        }
    }
}

fn install_essentials() -> Result<()> {
    cmd!("cargo", "install", "cargo-bloat").run()?;
    cmd!("rustup", "component", "add", "llvm-tools-preview").run()?;
    cmd!("cargo", "install", "grcov").run()?;
    Ok(())
}
