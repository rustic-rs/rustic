/// Show crate build times
use anyhow::Result;
use duct::cmd;

/// Show longest times taken in release build
///
/// # Errors
/// Errors if the command failed
///
pub fn bloat_time(package: impl Into<Option<String>>) -> Result<()> {
    let package = package.into().unwrap_or("rustic".to_string());
    cmd!("cargo", "bloat", "--time", "-j", "1", "-p", package).run()?;
    Ok(())
}

/// Show biggest crates in release build
///
/// # Errors
/// Errors if the command failed
///
pub fn bloat_deps(package: impl Into<Option<String>>) -> Result<()> {
    let package = package.into().unwrap_or("rustic".to_string());
    cmd!("cargo", "bloat", "--release", "--crates", "-p", package).run()?;
    Ok(())
}
