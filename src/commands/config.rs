//! `config` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::{get_repository, open_repository},
    status_err, Application, RUSTIC_APP,
};

use abscissa_core::{Command, Runnable, Shutdown};

use anyhow::{bail, Result};
use bytesize::ByteSize;

use rustic_core::{ConfigFile, DecryptBackend, DecryptWriteBackend};

/// `config` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct ConfigCmd {
    #[clap(flatten)]
    config_opts: ConfigOpts,
}

impl Runnable for ConfigCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl ConfigCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let mut repo = open_repository(get_repository(&config));

        let mut new_config = repo.config.clone();
        self.config_opts.apply(&mut new_config)?;

        if new_config == repo.config {
            println!("config is unchanged");
        } else {
            new_config.is_hot = None;
            // don't compress the config file
            repo.dbe.set_zstd(None);
            // for hot/cold backend, this only saves the config to the cold repo.
            _ = repo.dbe.save_file(&new_config)?;

            if let Some(hot_be) = repo.be_hot {
                // save config to hot repo
                let mut dbe = DecryptBackend::new(&hot_be, repo.key);
                // don't compress the config file
                dbe.set_zstd(None);
                new_config.is_hot = Some(true);
                _ = dbe.save_file(&new_config)?;
            }

            println!("saved new config");
        }

        Ok(())
    }
}

#[derive(clap::Parser, Debug)]
pub(crate) struct ConfigOpts {
    /// Set compression level. Allowed levels are 1 to 22 and -1 to -7, see <https://facebook.github.io/zstd/>.
    /// Note that 0 equals to no compression
    #[clap(long, value_name = "LEVEL")]
    pub(crate) set_compression: Option<i32>,

    /// Set repository version. Allowed versions: 1,2
    #[clap(long, value_name = "VERSION")]
    pub(crate) set_version: Option<u32>,

    /// Set default packsize for tree packs. rustic tries to always produce packs greater than this value.
    /// Note that for large repos, this value is grown by the grown factor.
    /// Defaults to 4 MiB if not set.
    #[clap(long, value_name = "SIZE")]
    pub(crate) set_treepack_size: Option<ByteSize>,

    /// Set upper limit for default packsize for tree packs.
    /// Note that packs actually can get up to some MiBs larger.
    /// If not set, pack sizes can grow up to approximately 4 GiB.
    #[clap(long, value_name = "SIZE")]
    pub(crate) set_treepack_size_limit: Option<ByteSize>,

    /// Set grow factor for tree packs. The default packsize grows by the square root of the total size of all
    /// tree packs multiplied with this factor. This means 32 kiB times this factor per square root of total
    /// treesize in GiB.
    /// Defaults to 32 (= 1MB per square root of total treesize in GiB) if not set.
    #[clap(long, value_name = "FACTOR")]
    pub(crate) set_treepack_growfactor: Option<u32>,

    /// Set default packsize for data packs. rustic tries to always produce packs greater than this value.
    /// Note that for large repos, this value is grown by the grown factor.
    /// Defaults to 32 MiB if not set.
    #[clap(long, value_name = "SIZE")]
    pub(crate) set_datapack_size: Option<ByteSize>,

    /// Set grow factor for data packs. The default packsize grows by the square root of the total size of all
    /// data packs multiplied with this factor. This means 32 kiB times this factor per square root of total
    /// datasize in GiB.
    /// Defaults to 32 (= 1MB per square root of total datasize in GiB) if not set.
    #[clap(long, value_name = "FACTOR")]
    pub(crate) set_datapack_growfactor: Option<u32>,

    /// Set upper limit for default packsize for tree packs.
    /// Note that packs actually can get up to some MiBs larger.
    /// If not set, pack sizes can grow up to approximately 4 GiB.
    #[clap(long, value_name = "SIZE")]
    pub(crate) set_datapack_size_limit: Option<ByteSize>,

    /// Set minimum tolerated packsize in percent of the targeted packsize.
    /// Defaults to 30 if not set.
    #[clap(long, value_name = "PERCENT")]
    pub(crate) set_min_packsize_tolerate_percent: Option<u32>,

    /// Set maximum tolerated packsize in percent of the targeted packsize
    /// A value of 0 means packs larger than the targeted packsize are always
    /// tolerated. Default if not set: larger packfiles are always tolerated.
    #[clap(long, value_name = "PERCENT")]
    pub(crate) set_max_packsize_tolerate_percent: Option<u32>,
}

impl ConfigOpts {
    pub(crate) fn apply(&self, config: &mut ConfigFile) -> Result<()> {
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
