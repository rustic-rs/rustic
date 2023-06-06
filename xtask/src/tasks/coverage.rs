use anyhow::Result;
use duct::cmd;
use std::{fs::create_dir_all, path::PathBuf};

pub fn coverage(
    devmode: bool,
    workspace: bool,
    target_dir: impl Into<Option<PathBuf>>,
) -> Result<()> {
    let mut target_dir_new = target_dir.into().unwrap_or(PathBuf::from("./target"));
    crate::helpers::remove_dir("coverage")?;
    create_dir_all("coverage")?;

    println!("=== running coverage ===");

    let base_cmd = if workspace {
        cmd!(
            "cargo",
            "test",
            "--workspace",
            "--target-dir",
            target_dir_new.clone()
        )
    } else {
        cmd!("cargo", "test", "--target-dir", target_dir_new.clone())
    };

    base_cmd
        .env("CARGO_INCREMENTAL", "0")
        .env("RUSTFLAGS", "-Cinstrument-coverage")
        .env("LLVM_PROFILE_FILE", "cargo-test-%p-%m.profraw")
        .run()?;
    println!("ok.");

    println!("=== generating report ===");
    let (fmt, file) = if devmode {
        ("html", "coverage/html")
    } else {
        ("lcov", "coverage/lcov.info")
    };

    target_dir_new = target_dir_new.join("debug").join("deps");

    cmd!(
        "grcov",
        ".",
        "--binary-path",
        target_dir_new,
        "-s",
        ".",
        "-t",
        fmt,
        "--branch",
        "--ignore-not-existing",
        "--ignore",
        "../*",
        "--ignore",
        "/*",
        "--ignore",
        "xtask/*",
        "--ignore",
        "*/src/tests/*",
        "-o",
        file,
    )
    .run()?;
    println!("ok.");

    println!("=== cleaning up ===");
    crate::helpers::clean_files("**/*.profraw")?;
    println!("ok.");
    if devmode {
        if crate::helpers::confirm("open report folder?") {
            cmd!("open", file).run()?;
        } else {
            println!("report location: {file}");
        }
    }

    Ok(())
}
