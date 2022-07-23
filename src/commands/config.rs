use anyhow::{bail, Result};
use bytesize::ByteSize;
use clap::Parser;

use crate::backend::{DecryptBackend, DecryptFullBackend, DecryptWriteBackend, WriteBackend};
use crate::repo::ConfigFile;

#[derive(Parser)]
pub(super) struct Opts {
    #[clap(flatten)]
    config_opts: ConfigOpts,
}

pub(super) async fn execute(
    be: &impl DecryptFullBackend,
    hot_be: &Option<impl WriteBackend>,
    opts: Opts,
    config: ConfigFile,
) -> Result<()> {
    let mut new_config = config.clone();
    opts.config_opts.apply(&mut new_config)?;
    if new_config != config {
        new_config.is_hot = None;
        // for hot/cold backend, this only saves the config to the cold repo.
        be.save_file(&new_config).await?;

        if let Some(hot_be) = hot_be {
            // save config to hot repo
            let dbe = DecryptBackend::new(hot_be, be.key().clone());
            new_config.is_hot = Some(true);
            dbe.save_file(&new_config).await?;
        }

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
    pub set_compression: Option<i32>,

    /// set repository version
    #[clap(long, value_name = "VERSION")]
    pub set_version: Option<u32>,

    /// Set default packsize for tree packs. rustic tries to always produce packs greater than this value.
    /// Note that for large repos, this value is grown by the grown factor.
    /// Defaults to 4 MiB if not set.
    #[clap(long, value_name = "SIZE")]
    pub set_treepack_size: Option<ByteSize>,

    /// Set grow factor for tree packs. The default packsize grows by the square root of the reposize
    /// multiplied with this factor. This means 32 kiB times this factor per square root of reposize in GiB.
    /// Defaults to 32 (= 1MB per sqare root of reposize in GiB) if not set.
    #[clap(long, value_name = "FACTOR")]
    pub set_treepack_growfactor: Option<u32>,

    /// Set default packsize for data packs. rustic tries to always produce packs greater than this value.
    /// Note that for large repos, this value is grown by the grown factor.
    /// Defaults to 32 MiB if not set.
    #[clap(long, value_name = "SIZE")]
    pub set_datapack_size: Option<ByteSize>,

    /// set grow factor for data packs. The default packsize grows by the square root of the reposize
    /// multiplied with this factor. This means 32 kiB times this factor per square root of reposize in GiB.
    /// Defaults to 32 (= 1MB per sqare root of reposize in GiB) if not set.
    #[clap(long, value_name = "FACTOR")]
    pub set_datapack_growfactor: Option<u32>,
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

        if let Some(size) = self.set_treepack_size {
            config.treepack_size = Some(size.as_u64().try_into()?);
        }
        if let Some(factor) = self.set_treepack_growfactor {
            config.treepack_growfactor = Some(factor);
        }
        if let Some(size) = self.set_datapack_size {
            config.datapack_size = Some(size.as_u64().try_into()?);
        }
        if let Some(factor) = self.set_treepack_growfactor {
            config.datapack_growfactor = Some(factor);
        }

        Ok(())
    }
}
