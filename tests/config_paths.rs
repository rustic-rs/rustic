//! Profile lookup tests using isolated child-process environments.

use std::{env, fs};

use assert_cmd::Command;
use predicates::prelude::*;
use rustic_testing::TestResult;
use tempfile::tempdir;

fn write_profile(directory: &std::path::Path, name: &str, repository: &str) -> TestResult<()> {
    let profile_dir = directory.join("rustic");
    fs::create_dir_all(&profile_dir)?;
    fs::write(
        profile_dir.join(format!("{name}.toml")),
        format!("[repository]\nrepository = \"{repository}\"\n"),
    )?;
    Ok(())
}

fn write_rustic_home_profile(
    rustic_home: &std::path::Path,
    name: &str,
    repository: &str,
) -> TestResult<()> {
    let config_dir = rustic_home.join("config");
    fs::create_dir_all(&config_dir)?;
    fs::write(
        config_dir.join(format!("{name}.toml")),
        format!("[repository]\nrepository = \"{repository}\"\n"),
    )?;
    Ok(())
}

fn show_config(profile: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rustic"));
    command
        .env_remove("RUSTIC_HOME")
        .env_remove("RUSTIC_REPOSITORY")
        .env_remove("RUSTIC_USE_PROFILE")
        .args(["--use-profile", profile, "show-config"]);
    command
}

#[test]
fn xdg_config_home_precedes_xdg_config_dirs() -> TestResult<()> {
    let temp_dir = tempdir()?;
    let config_home = temp_dir.path().join("config-home");
    let config_dir = temp_dir.path().join("config-dir");
    let profile = "xdg-home-precedence";
    write_profile(&config_home, profile, "home-marker")?;
    write_profile(&config_dir, profile, "directory-marker")?;

    show_config(profile)
        .current_dir(temp_dir.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_CONFIG_DIRS", env::join_paths([&config_dir])?)
        .assert()
        .success()
        .stdout(predicate::str::contains("repository = \"home-marker\""))
        .stdout(predicate::str::contains("directory-marker").not());

    Ok(())
}

#[test]
fn xdg_config_dirs_are_searched_in_order() -> TestResult<()> {
    let temp_dir = tempdir()?;
    let config_home = temp_dir.path().join("config-home-without-profile");
    let first_config_dir = temp_dir.path().join("first-config-dir");
    let second_config_dir = temp_dir.path().join("second-config-dir");
    let profile = "xdg-directory-order";
    write_profile(&first_config_dir, profile, "first-directory-marker")?;
    write_profile(&second_config_dir, profile, "second-directory-marker")?;

    show_config(profile)
        .current_dir(temp_dir.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .env(
            "XDG_CONFIG_DIRS",
            env::join_paths([&first_config_dir, &second_config_dir])?,
        )
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "repository = \"first-directory-marker\"",
        ))
        .stdout(predicate::str::contains("second-directory-marker").not());

    Ok(())
}

#[test]
fn rustic_home_precedes_xdg_config_directories() -> TestResult<()> {
    let temp_dir = tempdir()?;
    let rustic_home = temp_dir.path().join("rustic-home");
    let config_home = temp_dir.path().join("config-home");
    let config_dir = temp_dir.path().join("config-dir");
    let profile = "rustic-home-precedence";
    write_rustic_home_profile(&rustic_home, profile, "rustic-home-marker")?;
    write_profile(&config_home, profile, "xdg-home-marker")?;
    write_profile(&config_dir, profile, "xdg-directory-marker")?;

    show_config(profile)
        .current_dir(temp_dir.path())
        .env("RUSTIC_HOME", &rustic_home)
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_CONFIG_DIRS", env::join_paths([&config_dir])?)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "repository = \"rustic-home-marker\"",
        ))
        .stdout(predicate::str::contains("xdg-home-marker").not())
        .stdout(predicate::str::contains("xdg-directory-marker").not());

    Ok(())
}

#[test]
fn relative_rustic_home_is_ignored() -> TestResult<()> {
    let temp_dir = tempdir()?;
    let config_home = temp_dir.path().join("config-home");
    let relative_rustic_home = temp_dir.path().join("relative-rustic-home");
    let profile = "relative-rustic-home";
    write_rustic_home_profile(&relative_rustic_home, profile, "relative-marker")?;
    write_profile(&config_home, profile, "xdg-home-marker")?;

    show_config(profile)
        .current_dir(temp_dir.path())
        .env("RUSTIC_HOME", "relative-rustic-home")
        .env("XDG_CONFIG_HOME", &config_home)
        .env_remove("XDG_CONFIG_DIRS")
        .assert()
        .success()
        .stdout(predicate::str::contains("repository = \"xdg-home-marker\""))
        .stdout(predicate::str::contains("relative-marker").not());

    Ok(())
}
