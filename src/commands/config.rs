use anyhow::{bail, Result};
use clap::Parser;

use crate::backend::DecryptFullBackend;
use crate::repo::ConfigFile;

#[derive(Parser)]
pub(super) struct Opts {
    #[clap(flatten)]
    config_opts: ConfigOpts,
}

pub(super) async fn execute(
    be: &impl DecryptFullBackend,
    opts: Opts,
    config: ConfigFile,
) -> Result<()> {
    let mut new_config = config.clone();
    opts.config_opts.apply(&mut new_config)?;
    if new_config != config {
        be.save_file(&new_config).await?;
        println!("saved new config");
    } else {
        println!("config is unchanged");
    }

    Ok(())
}

#[derive(Parser)]
pub(super) struct ConfigOpts {
    /// set compression level, 0 equals no compression
    #[clap(long, value_name = "LEVEL")]
    set_compression: Option<i32>,

    /// set repository version
    #[clap(long, value_name = "VERSION")]
    set_version: Option<u32>,
}

impl ConfigOpts {
    pub fn apply(&self, config: &mut ConfigFile) -> Result<()> {
        if let Some(version) = self.set_version {
            let range = 1..=2;
            if !range.contains(&version) {
                bail!(
                    "version {version} is not supported. Allowed values: {}..{}",
                    range.start(),
                    range.end()
                );
            } else if version < config.version {
                bail!(
                    "cannot downgrade version from {} to {version}",
                    config.version
                );
            }
            config.version = version;
        }

        if let Some(compression) = self.set_compression {
            if config.version == 1 && compression != 0 {
                bail!("compression level {compression} is not supported for repo v1");
            }
            let range = zstd::compression_level_range();
            if !range.contains(&compression) {
                bail!(
                    "compression level {compression} is not supported. Allowed values: 0..{}",
                    range.end()
                );
            }
            config.compression = Some(compression);
        }

        Ok(())
    }
}
