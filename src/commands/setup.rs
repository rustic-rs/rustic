//! `setup` subcommand - interactive wizard for configuring rustic backups

use std::fs;
use std::path::{Path, PathBuf};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::{Result, anyhow, bail};
use dialoguer::{Confirm, Input, MultiSelect, Select, theme::ColorfulTheme};
use directories::ProjectDirs;
use log::{info, warn};

use crate::{Application, RUSTIC_APP, status_err};

/// `setup` subcommand - interactive wizard for configuring backups
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct SetupCmd {
    /// Profile name to create (without .toml extension).
    /// Leave empty or use the default 'rustic' for the default profile.
    #[clap(long, value_name = "PROFILE", default_value = "rustic")]
    profile: String,

    /// Overwrite existing profile if it exists
    #[clap(long)]
    force: bool,
}

impl Runnable for SetupCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        }
    }
}

// Common exclusion presets.

struct ExclusionPreset {
    name: &'static str,
    globs: Vec<&'static str>,
}

fn exclusion_presets() -> Vec<ExclusionPreset> {
    vec![
        ExclusionPreset {
            name: "Node.js (node_modules)",
            globs: vec!["!**/node_modules/**"],
        },
        ExclusionPreset {
            name: "Python (__pycache__, .venv, *.pyc)",
            globs: vec!["!**/__pycache__/**", "!**/.venv/**", "!**/*.pyc"],
        },
        ExclusionPreset {
            name: "Rust (target/)",
            globs: vec!["!**/target/**"],
        },
        ExclusionPreset {
            name: "Git (.git/)",
            globs: vec!["!**/.git/**"],
        },
        ExclusionPreset {
            name: "IDE files (.idea/, .vscode/, *.swp)",
            globs: vec!["!**/.idea/**", "!**/.vscode/**", "!**/*.swp"],
        },
        ExclusionPreset {
            name: "macOS (.DS_Store, ._*)",
            globs: vec!["!**/.DS_Store", "!**/._*"],
        },
        ExclusionPreset {
            name: "Temporary files (*.tmp, *.bak, *~)",
            globs: vec!["!**/*.tmp", "!**/*.bak", "!**/*~"],
        },
        ExclusionPreset {
            name: "Cache directories (CACHEDIR.TAG, .cache/)",
            globs: vec!["!**/.cache/**"],
        },
    ]
}

// Wizard implementation.

impl SetupCmd {
    fn inner_run(&self) -> Result<()> {
        let theme = ColorfulTheme::default();

        println!();
        println!("rustic setup");
        println!("============");
        println!("This wizard configures a backup source, target, and retention policy.");
        println!();

        // Ask for profile name
        let profile = loop {
            let input: String = Input::with_theme(&theme)
                .with_prompt("Profile name (leave empty for default)")
                .default(self.profile.clone())
                .allow_empty(true)
                .interact_text()?;
            let p = if input.trim().is_empty() {
                "rustic".to_string()
            } else {
                input.trim().to_string()
            };

            if let Err(err) = validate_profile_name(&p) {
                println!("Invalid profile name: {err}");
            } else {
                break p;
            }
        };
        println!();

        // Step 1: Repository target.

        println!("Step 1: Repository (where to store backups)");
        println!();

        let repo_types = vec![
            "Local path",
            "S3-compatible storage",
            "SFTP storage",
            "rclone remote",
            "REST server",
            "OpenDAL (advanced)",
        ];

        let repo_type_idx = Select::with_theme(&theme)
            .with_prompt("Where do you want to store backups?")
            .items(&repo_types)
            .default(0)
            .interact()?;

        let mut repo_options = toml::Table::new();

        let repository = match repo_type_idx {
            0 => {
                // Local path
                let path: String = Input::with_theme(&theme)
                    .with_prompt("Repository path")
                    .default("/backup/rustic".to_string())
                    .interact_text()?;
                let path = expand_tilde(&path);
                let repo_path = Path::new(&path);

                if !repo_path.exists() {
                    let create = Confirm::with_theme(&theme)
                        .with_prompt(format!("Directory '{}' doesn't exist. Create it?", path))
                        .default(true)
                        .interact()?;
                    if create {
                        fs::create_dir_all(repo_path)?;
                        info!("Created directory: {}", path);
                    }
                }

                path
            }
            1 => {
                // S3
                let bucket: String = Input::with_theme(&theme)
                    .with_prompt("S3 bucket name")
                    .interact_text()?;
                let root: String = Input::with_theme(&theme)
                    .with_prompt("Repository root inside bucket")
                    .default("/rustic".to_string())
                    .interact_text()?;
                let endpoint: String = Input::with_theme(&theme)
                    .with_prompt("S3 endpoint URL (optional, for non-AWS providers)")
                    .default(String::new())
                    .allow_empty(true)
                    .interact_text()?;
                let region: String = Input::with_theme(&theme)
                    .with_prompt("S3 region (optional)")
                    .default(String::new())
                    .allow_empty(true)
                    .interact_text()?;

                _ = repo_options.insert("bucket".to_string(), toml::Value::String(bucket));
                _ = repo_options.insert("root".to_string(), toml::Value::String(root));
                if !endpoint.trim().is_empty() {
                    _ = repo_options.insert(
                        "endpoint".to_string(),
                        toml::Value::String(endpoint.trim().to_string()),
                    );
                }
                if !region.trim().is_empty() {
                    _ = repo_options.insert(
                        "region".to_string(),
                        toml::Value::String(region.trim().to_string()),
                    );
                }

                "opendal:s3".to_string()
            }
            2 => {
                // SFTP
                let endpoint: String = Input::with_theme(&theme)
                    .with_prompt("SFTP endpoint (host:port)")
                    .interact_text()?;
                let user: String = Input::with_theme(&theme)
                    .with_prompt("SFTP user")
                    .interact_text()?;
                let root: String = Input::with_theme(&theme)
                    .with_prompt("Repository path on SFTP server")
                    .interact_text()?;

                _ = repo_options.insert("endpoint".to_string(), toml::Value::String(endpoint));
                _ = repo_options.insert("user".to_string(), toml::Value::String(user));
                _ = repo_options.insert("root".to_string(), toml::Value::String(root));

                "opendal:sftp".to_string()
            }
            3 => {
                // rclone
                let remote: String = Input::with_theme(&theme)
                    .with_prompt("rclone remote (e.g. myremote:backup/rustic)")
                    .interact_text()?;
                format!("rclone:{remote}")
            }
            4 => {
                // REST
                let url: String = Input::with_theme(&theme)
                    .with_prompt("REST server URL (e.g. http://localhost:8000)")
                    .default("http://localhost:8000".to_string())
                    .interact_text()?;
                format!("rest:{url}")
            }
            5 => {
                // OpenDAL advanced
                let service: String = Input::with_theme(&theme)
                    .with_prompt("OpenDAL service (e.g. s3, gcs, azblob, sftp)")
                    .interact_text()?;
                let options: String = Input::with_theme(&theme)
                    .with_prompt("OpenDAL options (key=value, comma-separated, optional)")
                    .default(String::new())
                    .allow_empty(true)
                    .interact_text()?;
                for option in options.split(',') {
                    let option = option.trim();
                    if option.is_empty() {
                        continue;
                    }
                    let Some((key, value)) = option.split_once('=') else {
                        bail!("invalid OpenDAL option '{option}', expected key=value");
                    };
                    _ = repo_options.insert(
                        key.trim().to_string(),
                        toml::Value::String(value.trim().to_string()),
                    );
                }
                format!("opendal:{}", service.trim())
            }
            _ => bail!("Invalid selection"),
        };

        // Password
        println!();
        let password_method = Select::with_theme(&theme)
            .with_prompt("How do you want to provide the repository password?")
            .items([
                "Always prompt (no stored password)",
                "Password file path",
                "Password command",
                "Type password now (stored in config)",
            ])
            .default(0)
            .interact()?;

        let (password, password_file, password_command) = match password_method {
            0 => (None, None, None),
            1 => {
                let file: String = Input::with_theme(&theme)
                    .with_prompt("Password file path")
                    .interact_text()?;
                (None, Some(file), None)
            }
            2 => {
                let cmd: String = Input::with_theme(&theme)
                    .with_prompt("Password command")
                    .interact_text()?;
                (None, None, Some(cmd))
            }
            3 => {
                warn!("The password will be stored in the generated profile.");
                let pass: String = dialoguer::Password::with_theme(&theme)
                    .with_prompt("Repository password")
                    .allow_empty_password(true)
                    .with_confirmation("Confirm password", "Passwords do not match")
                    .interact()?;
                // Password strength feedback
                if pass.is_empty() {
                    warn!("Warning: Empty password. The repository data will still be encrypted,");
                    warn!("  but anyone with access to the repository can decrypt it.");
                } else {
                    let strength = password_strength::estimate_strength(&pass);
                    if strength < 0.7 {
                        warn!(
                            "Warning: Your password is rated as weak ({:.2}/1.0).",
                            strength
                        );
                        warn!("  Consider using a stronger password for better security.");
                    }
                }
                (Some(pass), None, None)
            }
            _ => bail!("Invalid selection"),
        };

        // Step 2: Backup sources.

        println!();
        println!("Step 2: Backup Sources (what to back up)");
        println!();

        let mut sources: Vec<Vec<String>> = Vec::new();
        loop {
            let source: String = Input::with_theme(&theme)
                .with_prompt("Add a path to back up (or press Enter to finish)")
                .default(String::new())
                .allow_empty(true)
                .interact_text()?;

            let source = source.trim().to_string();

            if source.is_empty() {
                if sources.is_empty() {
                    println!("You need at least one backup source.");
                    continue;
                }
                break;
            }

            let expanded = expand_tilde(&source);
            if !Path::new(&expanded).exists() {
                println!("Warning: '{}' does not exist.", expanded);
                let add_anyway = Confirm::with_theme(&theme)
                    .with_prompt("Add it anyway?")
                    .default(false)
                    .interact()?;
                if !add_anyway {
                    continue;
                }
            }
            sources.push(vec![expanded]);
            println!("Added: {}", source);
        }

        // Exclusion patterns
        println!();
        let use_exclusions = Confirm::with_theme(&theme)
            .with_prompt("Configure exclusion patterns?")
            .default(true)
            .interact()?;

        let mut globs: Vec<String> = Vec::new();
        let mut exclude_if_present: Vec<String> = Vec::new();
        let mut use_git_ignore = false;

        if use_exclusions {
            // Preset exclusions
            let presets = exclusion_presets();
            let preset_names: Vec<&str> = presets.iter().map(|p| p.name).collect();

            let selections = MultiSelect::with_theme(&theme)
                .with_prompt("Select exclusion presets (Space to toggle, Enter to confirm)")
                .items(&preset_names)
                .interact()?;

            for idx in selections {
                for glob in &presets[idx].globs {
                    globs.push(glob.to_string());
                }
            }

            // Git ignore
            use_git_ignore = Confirm::with_theme(&theme)
                .with_prompt("Respect .gitignore files?")
                .default(true)
                .interact()?;

            // Common exclude-if-present markers
            let use_nobackup = Confirm::with_theme(&theme)
                .with_prompt("Exclude directories containing '.nobackup' or 'CACHEDIR.TAG'?")
                .default(true)
                .interact()?;

            if use_nobackup {
                exclude_if_present.push(".nobackup".to_string());
                exclude_if_present.push("CACHEDIR.TAG".to_string());
            }

            // Custom globs
            let custom_globs: String = Input::with_theme(&theme)
                .with_prompt("Custom exclusion globs (comma-separated, or Enter to skip)")
                .default(String::new())
                .allow_empty(true)
                .interact_text()?;

            if !custom_globs.is_empty() {
                for g in custom_globs.split(',') {
                    let g = g.trim();
                    if !g.is_empty() {
                        globs.push(format!("!{g}"));
                    }
                }
            }
        }

        // Step 3: Retention policy.

        println!();
        println!("Step 3: Retention Policy (how long to keep backups)");
        println!();

        let retention_presets = vec![
            "Conservative (keep-daily=7, keep-weekly=4, keep-monthly=12, keep-yearly=5)",
            "Moderate (keep-daily=3, keep-weekly=2, keep-monthly=6)",
            "Minimal (keep-daily=1, keep-weekly=1, keep-monthly=3)",
            "Custom",
            "None (manual forget only)",
        ];

        let retention_idx = Select::with_theme(&theme)
            .with_prompt("Select a retention policy")
            .items(&retention_presets)
            .default(0)
            .interact()?;

        struct RetentionConfig {
            keep_daily: u32,
            keep_weekly: u32,
            keep_monthly: u32,
            keep_yearly: u32,
        }

        let retention = match retention_idx {
            0 => Some(RetentionConfig {
                keep_daily: 7,
                keep_weekly: 4,
                keep_monthly: 12,
                keep_yearly: 5,
            }),
            1 => Some(RetentionConfig {
                keep_daily: 3,
                keep_weekly: 2,
                keep_monthly: 6,
                keep_yearly: 0,
            }),
            2 => Some(RetentionConfig {
                keep_daily: 1,
                keep_weekly: 1,
                keep_monthly: 3,
                keep_yearly: 0,
            }),
            3 => {
                let daily: u32 = Input::with_theme(&theme)
                    .with_prompt("Keep daily snapshots")
                    .default(7)
                    .interact_text()?;
                let weekly: u32 = Input::with_theme(&theme)
                    .with_prompt("Keep weekly snapshots")
                    .default(4)
                    .interact_text()?;
                let monthly: u32 = Input::with_theme(&theme)
                    .with_prompt("Keep monthly snapshots")
                    .default(12)
                    .interact_text()?;
                let yearly: u32 = Input::with_theme(&theme)
                    .with_prompt("Keep yearly snapshots")
                    .default(5)
                    .interact_text()?;
                Some(RetentionConfig {
                    keep_daily: daily,
                    keep_weekly: weekly,
                    keep_monthly: monthly,
                    keep_yearly: yearly,
                })
            }
            _ => None,
        };

        // Step 4: Performance options.

        println!();
        println!("Step 4: Performance Options");
        println!();

        let compression_idx = Select::with_theme(&theme)
            .with_prompt("Select compression level")
            .items(["Default", "None", "Max"])
            .default(0)
            .interact()?;

        let compression = match compression_idx {
            1 => Some("0"),  // Level 0 represents no compression in rustic config
            2 => Some("22"), // Max compression level for zstd is 22
            _ => None,       // null implies auto/default in rustic
        };

        let pack_size_preset = Select::with_theme(&theme)
            .with_prompt("Select default pack size")
            .items(["Default", "Large", "Extra Large"])
            .default(0)
            .interact()?;

        let pack_size = match pack_size_preset {
            1 => Some(128_u32),
            2 => Some(512_u32),
            _ => None,
        };

        // Step 5: Generate config and summary.

        println!();
        println!("Step 5: Summary & Configuration");
        println!();

        // Build the TOML config string using the toml crate for safety and robustness
        let mut config_table = toml::Table::new();

        // Repository section
        let mut repo_table = toml::Table::new();
        _ = repo_table.insert(
            "repository".to_string(),
            toml::Value::String(repository.clone()),
        );
        if let Some(pass) = password {
            _ = repo_table.insert("password".to_string(), toml::Value::String(pass));
        }
        if let Some(file) = password_file {
            _ = repo_table.insert("password-file".to_string(), toml::Value::String(file));
        }
        if let Some(cmd) = password_command {
            _ = repo_table.insert("password-command".to_string(), toml::Value::String(cmd));
        }
        if !repo_options.is_empty() {
            _ = repo_table.insert("options".to_string(), toml::Value::Table(repo_options));
        }
        _ = config_table.insert("repository".to_string(), toml::Value::Table(repo_table));

        // Backup section
        let mut backup_table = toml::Table::new();
        if use_git_ignore {
            _ = backup_table.insert("git-ignore".to_string(), toml::Value::Boolean(true));
        }
        if !exclude_if_present.is_empty() {
            _ = backup_table.insert(
                "exclude-if-present".to_string(),
                toml::Value::Array(
                    exclude_if_present
                        .iter()
                        .cloned()
                        .map(toml::Value::String)
                        .collect(),
                ),
            );
        }
        if !globs.is_empty() {
            _ = backup_table.insert(
                "globs".to_string(),
                toml::Value::Array(globs.iter().cloned().map(toml::Value::String).collect()),
            );
        }

        // Backup sources (via snapshots array of tables)
        let mut snapshots = toml::value::Array::new();
        for source_paths in &sources {
            let mut snap_table = toml::Table::new();
            _ = snap_table.insert(
                "sources".to_string(),
                toml::Value::Array(
                    source_paths
                        .iter()
                        .cloned()
                        .map(toml::Value::String)
                        .collect(),
                ),
            );
            snapshots.push(toml::Value::Table(snap_table));
        }
        if !snapshots.is_empty() {
            _ = backup_table.insert("snapshots".to_string(), toml::Value::Array(snapshots));
        }
        _ = config_table.insert("backup".to_string(), toml::Value::Table(backup_table));

        // Forget/retention section
        if let Some(ref ret) = retention {
            let mut forget_table = toml::Table::new();
            if ret.keep_daily > 0 {
                _ = forget_table.insert(
                    "keep-daily".to_string(),
                    toml::Value::Integer(i64::from(ret.keep_daily)),
                );
            }
            if ret.keep_weekly > 0 {
                _ = forget_table.insert(
                    "keep-weekly".to_string(),
                    toml::Value::Integer(i64::from(ret.keep_weekly)),
                );
            }
            if ret.keep_monthly > 0 {
                _ = forget_table.insert(
                    "keep-monthly".to_string(),
                    toml::Value::Integer(i64::from(ret.keep_monthly)),
                );
            }
            if ret.keep_yearly > 0 {
                _ = forget_table.insert(
                    "keep-yearly".to_string(),
                    toml::Value::Integer(i64::from(ret.keep_yearly)),
                );
            }
            _ = config_table.insert("forget".to_string(), toml::Value::Table(forget_table));
        }

        let mut config = format!(
            "# rustic config profile: {}\n# Generated by 'rustic setup' on {}\n\n",
            profile,
            jiff::Zoned::now().strftime("%Y-%m-%d %H:%M:%S")
        );
        config.push_str(&toml::to_string_pretty(&config_table).map_err(|e| anyhow!(e))?);

        // Print summary
        println!("Configuration summary:");
        println!("  Profile:    {}", profile);
        println!("  Repository: {}", truncate_str(&repository, 64));
        println!(
            "  Sources:    {}",
            if sources.len() == 1 {
                truncate_str(&sources[0].join(", "), 64)
            } else {
                format!("{} paths configured", sources.len())
            }
        );
        if !globs.is_empty() {
            println!("  Exclusions: {} patterns", globs.len());
        }
        if let Some(ref ret) = retention {
            println!(
                "  Retention:  daily={}, weekly={}, monthly={}, yearly={}",
                ret.keep_daily, ret.keep_weekly, ret.keep_monthly, ret.keep_yearly
            );
        }
        println!();

        // Show generated config
        println!("Generated configuration:");
        println!("------------------------");
        println!("{config}");
        println!("------------------------");
        println!();

        // Determine config path
        let config_dir = ProjectDirs::from("", "", "rustic")
            .map(|dirs| dirs.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let config_file = config_dir.join(format!("{}.toml", profile));

        // Check for existing config
        if config_file.exists() && !self.force {
            let overwrite = Confirm::with_theme(&theme)
                .with_prompt(format!(
                    "Config file '{}' already exists. Overwrite?",
                    config_file.display()
                ))
                .default(false)
                .interact()?;
            if !overwrite {
                println!("Aborted. Use --force to overwrite.");
                return Ok(());
            }
        }

        // Save the config
        let save = Confirm::with_theme(&theme)
            .with_prompt(format!(
                "Save configuration to '{}'?",
                config_file.display()
            ))
            .default(true)
            .interact()?;

        if save {
            fs::create_dir_all(&config_dir)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .mode(0o600)
                    .open(&config_file)?;
                std::io::Write::write_all(&mut file, config.as_bytes())?;
                fs::set_permissions(&config_file, fs::Permissions::from_mode(0o600))?;
            }
            #[cfg(not(unix))]
            {
                fs::write(&config_file, &config)?;
            }

            println!();
            println!("Configuration saved to: {}", config_file.display());
        } else {
            println!("Configuration not saved.");
            return Ok(());
        }

        // Offer to initialize the repository
        println!();
        let init_repo = Confirm::with_theme(&theme)
            .with_prompt("Initialize the repository now?")
            .default(true)
            .interact()?;

        if init_repo {
            let profile_arg = if profile == "rustic" {
                String::new()
            } else {
                format!(" -P {}", profile)
            };

            let mut init_args = String::new();
            if let Some(comp) = compression {
                init_args.push_str(&format!(" --set-compression {}", comp));
            }
            if let Some(size) = pack_size {
                init_args.push_str(&format!(" --set-datapack-size {}MiB", size));
                init_args.push_str(&format!(
                    " --set-treepack-size {}MiB",
                    if size > 32 { 16 } else { 4 }
                ));
            }

            println!();
            println!("Run the following command to initialize:");
            println!("  rustic{profile_arg} init{init_args}");
            println!();
            println!("Then start your first backup with:");
            println!("  rustic{profile_arg} backup");
        }

        // Usage hints
        println!();
        println!("Next steps:");
        println!();
        if profile != "rustic" {
            println!(
                "  Use -P {} with all rustic commands to use this profile:",
                profile
            );
            println!("    rustic -P {} init", profile);
            println!("    rustic -P {} backup", profile);
            println!("    rustic -P {} snapshots", profile);
            println!("    rustic -P {} restore latest /restore/path", profile);
        } else {
            println!("  This is the default profile. Commands:");
            println!("    rustic init");
            println!("    rustic backup");
            println!("    rustic snapshots");
            println!("    rustic restore latest /restore/path");
        }
        println!();
        println!("  For more information: https://rustic.cli.rs/docs/getting_started.html");
        println!();

        Ok(())
    }
}

/// Truncate a string to a max length (by characters), adding "..." if truncated
fn truncate_str(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", s.chars().take(max_len - 3).collect::<String>())
    } else {
        s.chars().take(max_len).collect::<String>()
    }
}

fn validate_profile_name(profile: &str) -> Result<()> {
    if profile.is_empty() {
        bail!("name cannot be empty");
    }
    if matches!(profile, "." | "..") {
        bail!("name cannot be '.' or '..'");
    }
    if profile.ends_with(".toml") {
        bail!("enter the profile name without the .toml extension");
    }
    if profile
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        Ok(())
    } else {
        bail!("use only ASCII letters, numbers, '.', '-' and '_'");
    }
}

/// Simple tilde expansion: replace leading `~` with the user's home directory
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix('~')
        && let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
    {
        let home = home.to_string_lossy();
        if rest.is_empty() {
            return home.to_string();
        }
        if rest.starts_with('/') || rest.starts_with('\\') {
            return format!("{home}{rest}");
        }
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::{truncate_str, validate_profile_name};

    #[test]
    fn profile_name_validation_accepts_safe_names() {
        for profile in ["rustic", "daily-backup", "home_1", "prod.eu"] {
            validate_profile_name(profile).unwrap();
        }
    }

    #[test]
    fn profile_name_validation_rejects_paths_and_extensions() {
        for profile in [
            "",
            ".",
            "..",
            "../rustic",
            "nested/profile",
            "nested\\profile",
            "a.toml",
        ] {
            assert!(validate_profile_name(profile).is_err());
        }
    }

    #[test]
    fn truncate_str_is_utf8_safe() {
        let sample = "\u{e5}\u{df}\u{2202}\u{192}";
        assert_eq!(truncate_str("abcdef", 4), "a...");
        assert_eq!(truncate_str(sample, 3), "\u{e5}\u{df}\u{2202}");
        assert_eq!(truncate_str(sample, 4), sample);
    }
}
