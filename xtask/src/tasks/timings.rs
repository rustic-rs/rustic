use anyhow::{Ok, Result};
use duct::cmd;

pub fn timings(json: bool) -> Result<()> {
    let mut timings_str = String::from("--timings=");

    if json {
        timings_str.push_str("json")
    } else {
        timings_str.push_str("html")
    }

    cmd!("cargo", "build", "--release", timings_str).run()?;
    Ok(())
}
