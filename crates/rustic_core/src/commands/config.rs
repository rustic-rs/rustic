//! `config` subcommand
use bytesize::ByteSize;

use crate::{
    backend::decrypt::DecryptBackend, error::CommandErrorKind, ConfigFile, DecryptWriteBackend,
    Key, Open, Repository, RusticResult,
};

pub(crate) fn apply_config<P, S: Open>(
    repo: &Repository<P, S>,
    opts: &ConfigOpts,
) -> RusticResult<bool> {
    let mut new_config = repo.config().clone();
    opts.apply(&mut new_config)?;
    if &new_config == repo.config() {
        Ok(false)
    } else {
        save_config(repo, new_config, *repo.key())?;
        Ok(true)
    }
}

pub(crate) fn save_config<P, S>(
    repo: &Repository<P, S>,
    mut new_config: ConfigFile,
    key: Key,
) -> RusticResult<()> {
    new_config.is_hot = None;
    // don't compress the config file
    let mut dbe = DecryptBackend::new(&repo.be, key);
    dbe.set_zstd(None);
    // for hot/cold backend, this only saves the config to the cold repo.
    _ = dbe.save_file(&new_config)?;

    if let Some(hot_be) = repo.be_hot.clone() {
        // save config to hot repo
        let mut dbe = DecryptBackend::new(&hot_be, key);
        // don't compress the config file
        dbe.set_zstd(None);
        new_config.is_hot = Some(true);
        _ = dbe.save_file(&new_config)?;
    }
    Ok(())
}

#[cfg_attr(feature = "clap", derive(clap::Parser))]
#[derive(Debug, Clone, Copy, Default)]
pub struct ConfigOpts {
    /// Set compression level. Allowed levels are 1 to 22 and -1 to -7, see <https://facebook.github.io/zstd/>.
    /// Note that 0 equals to no compression
    #[cfg_attr(feature = "clap", clap(long, value_name = "LEVEL"))]
    pub set_compression: Option<i32>,

    /// Set repository version. Allowed versions: 1,2
    #[cfg_attr(feature = "clap", clap(long, value_name = "VERSION"))]
    pub set_version: Option<u32>,

    /// Set default packsize for tree packs. rustic tries to always produce packs greater than this value.
    /// Note that for large repos, this value is grown by the grown factor.
    /// Defaults to 4 MiB if not set.
    #[cfg_attr(feature = "clap", clap(long, value_name = "SIZE"))]
    pub set_treepack_size: Option<ByteSize>,

    /// Set upper limit for default packsize for tree packs.
    /// Note that packs actually can get up to some MiBs larger.
    /// If not set, pack sizes can grow up to approximately 4 GiB.
    #[cfg_attr(feature = "clap", clap(long, value_name = "SIZE"))]
    pub set_treepack_size_limit: Option<ByteSize>,

    /// Set grow factor for tree packs. The default packsize grows by the square root of the total size of all
    /// tree packs multiplied with this factor. This means 32 kiB times this factor per square root of total
    /// treesize in GiB.
    /// Defaults to 32 (= 1MB per square root of total treesize in GiB) if not set.
    #[cfg_attr(feature = "clap", clap(long, value_name = "FACTOR"))]
    pub set_treepack_growfactor: Option<u32>,

    /// Set default packsize for data packs. rustic tries to always produce packs greater than this value.
    /// Note that for large repos, this value is grown by the grown factor.
    /// Defaults to 32 MiB if not set.
    #[cfg_attr(feature = "clap", clap(long, value_name = "SIZE"))]
    pub set_datapack_size: Option<ByteSize>,

    /// Set grow factor for data packs. The default packsize grows by the square root of the total size of all
    /// data packs multiplied with this factor. This means 32 kiB times this factor per square root of total
    /// datasize in GiB.
    /// Defaults to 32 (= 1MB per square root of total datasize in GiB) if not set.
    #[cfg_attr(feature = "clap", clap(long, value_name = "FACTOR"))]
    pub set_datapack_growfactor: Option<u32>,

    /// Set upper limit for default packsize for tree packs.
    /// Note that packs actually can get up to some MiBs larger.
    /// If not set, pack sizes can grow up to approximately 4 GiB.
    #[cfg_attr(feature = "clap", clap(long, value_name = "SIZE"))]
    pub set_datapack_size_limit: Option<ByteSize>,

    /// Set minimum tolerated packsize in percent of the targeted packsize.
    /// Defaults to 30 if not set.
    #[cfg_attr(feature = "clap", clap(long, value_name = "PERCENT"))]
    pub set_min_packsize_tolerate_percent: Option<u32>,

    /// Set maximum tolerated packsize in percent of the targeted packsize
    /// A value of 0 means packs larger than the targeted packsize are always
    /// tolerated. Default if not set: larger packfiles are always tolerated.
    #[cfg_attr(feature = "clap", clap(long, value_name = "PERCENT"))]
    pub set_max_packsize_tolerate_percent: Option<u32>,
}

impl ConfigOpts {
    pub fn apply(&self, config: &mut ConfigFile) -> RusticResult<()> {
        if let Some(version) = self.set_version {
            let range = 1..=2;
            if !range.contains(&version) {
                return Err(CommandErrorKind::VersionNotSupported(version, range).into());
            } else if version < config.version {
                return Err(CommandErrorKind::CannotDowngrade(config.version, version).into());
            }
            config.version = version;
        }

        if let Some(compression) = self.set_compression {
            if config.version == 1 && compression != 0 {
                return Err(CommandErrorKind::NoCompressionV1Repo(compression).into());
            }
            let range = zstd::compression_level_range();
            if !range.contains(&compression) {
                return Err(
                    CommandErrorKind::CompressionLevelNotSupported(compression, range).into(),
                );
            }
            config.compression = Some(compression);
        }

        if let Some(size) = self.set_treepack_size {
            config.treepack_size = Some(
                size.as_u64()
                    .try_into()
                    .map_err(|_| CommandErrorKind::SizeTooLarge(size))?,
            );
        }
        if let Some(factor) = self.set_treepack_growfactor {
            config.treepack_growfactor = Some(factor);
        }
        if let Some(size) = self.set_treepack_size_limit {
            config.treepack_size_limit = Some(
                size.as_u64()
                    .try_into()
                    .map_err(|_| CommandErrorKind::SizeTooLarge(size))?,
            );
        }

        if let Some(size) = self.set_datapack_size {
            config.datapack_size = Some(
                size.as_u64()
                    .try_into()
                    .map_err(|_| CommandErrorKind::SizeTooLarge(size))?,
            );
        }
        if let Some(factor) = self.set_datapack_growfactor {
            config.datapack_growfactor = Some(factor);
        }
        if let Some(size) = self.set_datapack_size_limit {
            config.datapack_size_limit = Some(
                size.as_u64()
                    .try_into()
                    .map_err(|_| CommandErrorKind::SizeTooLarge(size))?,
            );
        }

        if let Some(percent) = self.set_min_packsize_tolerate_percent {
            if percent > 100 {
                return Err(CommandErrorKind::MinPackSizeTolerateWrong.into());
            }
            config.min_packsize_tolerate_percent = Some(percent);
        }

        if let Some(percent) = self.set_max_packsize_tolerate_percent {
            if percent < 100 && percent > 0 {
                return Err(CommandErrorKind::MaxPackSizeTolerateWrong.into());
            }
            config.max_packsize_tolerate_percent = Some(percent);
        }

        Ok(())
    }
}
