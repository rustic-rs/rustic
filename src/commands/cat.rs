//! `cat` subcommand

use crate::{Application, RUSTIC_APP, status_err};

use abscissa_core::{Command, Runnable, Shutdown};

use anyhow::{Context, Result};

use rustic_core::repofile::{BlobType, ConfigFile, FileType};
use serde_json::{Map, Value};

/// `cat` subcommand
///
/// Output the contents of a file or blob
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct CatCmd {
    #[clap(subcommand)]
    cmd: CatSubCmd,
}

/// `cat` subcommands
#[derive(clap::Subcommand, Debug)]
enum CatSubCmd {
    /// Display a tree blob
    TreeBlob(IdOpt),
    /// Display a data blob
    DataBlob(IdOpt),
    /// Display the config file, including effective defaults
    Config,
    /// Display an index file
    Index(IdOpt),
    /// Display a snapshot file
    Snapshot(IdOpt),
    /// Display a tree within a snapshot
    Tree(TreeOpts),
    /// Display the masterkey
    Masterkey,
}

#[derive(Default, clap::Parser, Debug)]
struct IdOpt {
    /// Id to display
    id: String,
}

#[derive(clap::Parser, Debug)]
struct TreeOpts {
    /// Snapshot/path of the tree to display
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,
}

impl Runnable for CatCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl CatCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let data = match &self.cmd {
            CatSubCmd::Config => config.repository.run_open(|repo| {
                let raw_config = repo.cat_file(FileType::Config, "")?;
                Ok(config_with_effective_defaults(repo.config(), &raw_config)?.into())
            })?,
            CatSubCmd::Index(opt) => config
                .repository
                .run_open(|repo| Ok(repo.cat_file(FileType::Index, &opt.id)?))?,
            CatSubCmd::Snapshot(opt) => config
                .repository
                .run_open(|repo| Ok(repo.cat_file(FileType::Snapshot, &opt.id)?))?,
            CatSubCmd::TreeBlob(opt) => config
                .repository
                .run_indexed(|repo| Ok(repo.cat_blob(BlobType::Tree, &opt.id)?))?,
            CatSubCmd::DataBlob(opt) => config
                .repository
                .run_indexed(|repo| Ok(repo.cat_blob(BlobType::Data, &opt.id)?))?,
            CatSubCmd::Tree(opt) => config.repository.run_indexed(|repo| {
                Ok(repo.cat_tree(&opt.snap, |sn| config.snapshot_filter.matches(sn))?)
            })?,
            CatSubCmd::Masterkey => config
                .repository
                .run_open(|repo| Ok(serde_json::to_vec(&repo.key())?.into()))?,
        };
        println!("{}", String::from_utf8(data.to_vec())?);

        Ok(())
    }
}

/// Add the values used for repository settings that are absent from the stored
/// config file.
///
/// The raw config remains at the top level. Values in `_effective_defaults`
/// are informational only: writing them back would turn future defaults into
/// fixed configuration values.
fn config_with_effective_defaults(config: &ConfigFile, raw_config: &[u8]) -> Result<Vec<u8>> {
    let mut raw_config: Map<String, Value> =
        serde_json::from_slice(raw_config).context("repository config is not a JSON object")?;
    let mut defaults = Map::new();

    let mut add_default = |name: &str, value: Value| {
        if !raw_config.contains_key(name) {
            _ = defaults.insert(name.to_string(), value);
        }
    };

    add_default("chunker", serde_json::to_value(config.chunker())?);
    add_default("chunk_size", Value::from(config.chunk_size()));
    add_default("chunk_min_size", Value::from(config.chunk_min_size()));
    add_default("chunk_max_size", Value::from(config.chunk_max_size()));
    add_default("is_hot", Value::from(config.is_hot.unwrap_or(false)));
    add_default(
        "append_only",
        Value::from(config.append_only.unwrap_or(false)),
    );

    let compression = match config.zstd()? {
        Some(_) => "zstd-default",
        None => "none",
    };
    add_default("compression", Value::from(compression));

    let (treepack_size, treepack_growfactor, treepack_size_limit) = config.packsize(BlobType::Tree);
    add_default("treepack_size", Value::from(treepack_size));
    add_default("treepack_growfactor", Value::from(treepack_growfactor));
    add_default("treepack_size_limit", Value::from(treepack_size_limit));

    let (datapack_size, datapack_growfactor, datapack_size_limit) = config.packsize(BlobType::Data);
    add_default("datapack_size", Value::from(datapack_size));
    add_default("datapack_growfactor", Value::from(datapack_growfactor));
    add_default("datapack_size_limit", Value::from(datapack_size_limit));

    let (min_packsize_tolerate_percent, max_packsize_tolerate_percent) =
        config.packsize_ok_percents();
    add_default(
        "min_packsize_tolerate_percent",
        Value::from(min_packsize_tolerate_percent),
    );
    add_default(
        "max_packsize_tolerate_percent",
        if max_packsize_tolerate_percent == u32::MAX {
            Value::from("unlimited")
        } else {
            Value::from(max_packsize_tolerate_percent)
        },
    );
    add_default("extra_verify", Value::from(config.extra_verify()));

    drop(add_default);
    _ = raw_config.insert("_effective_defaults".to_string(), Value::Object(defaults));
    Ok(serde_json::to_vec(&raw_config)?)
}
