use anyhow::{bail, Result};
use bytesize::ByteSize;
use clap::{AppSettings, Parser};

use crate::backend::{DecryptBackend, DecryptFullBackend, DecryptWriteBackend, WriteBackend};
use crate::repofile::ConfigFile;

#[derive(Parser)]
pub(super) struct Opts {
    #[clap(flatten)]
    config_opts: ConfigOpts,
}

pub(super) fn execute(
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
        be.save_file(&new_config)?;

        if let Some(hot_be) = hot_be {
            // save config to hot repo
            let dbe = DecryptBackend::new(hot_be, be.key().clone());
            new_config.is_hot = Some(true);
            dbe.save_file(&new_config)?;
        }

        println!("saved new config");
    } else {
        println!("config is unchanged");
    }

    Ok(())
}

#[derive(Parser)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
pub(super) struct ConfigOpts {
    /// Set compression level. Allowed levels are 1 to 22 and -1 to -7, see https://facebook.github.io/zstd/.
    /// Note that 0 equals to no compression
    #[clap(long, value_name = "LEVEL")]
    pub set_compression: Option<i32>,

    /// Set repository version. Allowed versions: 1,2
    #[clap(long, value_name = "VERSION")]
    pub set_version: Option<u32>,

    /// Set default packsize for tree packs. rustic tries to always produce packs greater than this value.
    /// Note that for large repos, this value is grown by the grown factor.
    /// Defaults to 4 MiB if not set.
    #[clap(long, value_name = "SIZE")]
    pub set_treepack_size: Option<ByteSize>,

    /// Set upper limit for default packsize for tree packs.
    /// Note that packs actually can get up to some MiBs larger.
    /// If not set, pack sizes can grow up to approximately 4 GiB.
    #[clap(long, value_name = "SIZE")]
    pub set_treepack_size_limit: Option<ByteSize>,

    /// Set grow factor for tree packs. The default packsize grows by the square root of the total size of all
    /// tree packs multiplied with this factor. This means 32 kiB times this factor per square root of total
    /// treesize in GiB.
    /// Defaults to 32 (= 1MB per sqare root of total treesize in GiB) if not set.
    #[clap(long, value_name = "FACTOR")]
    pub set_treepack_growfactor: Option<u32>,

    /// Set default packsize for data packs. rustic tries to always produce packs greater than this value.
    /// Note that for large repos, this value is grown by the grown factor.
    /// Defaults to 32 MiB if not set.
    #[clap(long, value_name = "SIZE")]
    pub set_datapack_size: Option<ByteSize>,

    /// Set grow factor for data packs. The default packsize grows by the square root of the total size of all
    /// data packs multiplied with this factor. This means 32 kiB times this factor per square root of total
    /// datasize in GiB.
    /// Defaults to 32 (= 1MB per sqare root of total datasize in GiB) if not set.
    #[clap(long, value_name = "FACTOR")]
    pub set_datapack_growfactor: Option<u32>,

    /// Set upper limit for default packsize for tree packs.
    /// Note that packs actually can get up to some MiBs larger.
    /// If not set, pack sizes can grow up to approximately 4 GiB.
    #[clap(long, value_name = "SIZE")]
    pub set_datapack_size_limit: Option<ByteSize>,

    /// Set minimum tolerated packsize in percent of the targeted packsize.
    /// Defaults to 30 if not set.
    #[clap(long, value_name = "PERCENT")]
    pub set_min_packsize_tolerate_percent: Option<u32>,

    /// Set maximum tolerated packsize in percent of the targeted packsize
    /// A value of 0 means packs larger than the targeted packsize are always
    /// tolerated. Default if not set: larger packfiles are always tolerated.
    #[clap(long, value_name = "PERCENT")]
    pub set_max_packsize_tolerate_percent: Option<u32>,
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
        if let Some(size) = self.set_treepack_size_limit {
            config.treepack_size_limit = Some(size.as_u64().try_into()?);
        }

        if let Some(size) = self.set_datapack_size {
            config.datapack_size = Some(size.as_u64().try_into()?);
        }
        if let Some(factor) = self.set_datapack_growfactor {
            config.datapack_growfactor = Some(factor);
        }
        if let Some(size) = self.set_datapack_size_limit {
            config.datapack_size_limit = Some(size.as_u64().try_into()?);
        }

        if let Some(percent) = self.set_min_packsize_tolerate_percent {
            if percent > 100 {
                bail!("set_min_packsize_tolerate_percent must be <= 100");
            }
            config.min_packsize_tolerate_percent = Some(percent);
        }

        if let Some(percent) = self.set_max_packsize_tolerate_percent {
            if percent < 100 && percent > 0 {
                bail!("set_max_packsize_tolerate_percent must be >= 100 or 0");
            }
            config.max_packsize_tolerate_percent = Some(percent);
        }

        Ok(())
    }
}
