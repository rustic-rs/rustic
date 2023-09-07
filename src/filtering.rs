use crate::error::RhaiErrorKinds;

use log::warn;
use rustic_core::{repofile::SnapshotFile, StringList};
use std::{error::Error, str::FromStr};

use rhai::{serde::to_dynamic, Dynamic, Engine, FnPtr, AST};
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};

#[derive(Clone, Debug)]
pub(crate) struct SnapshotFn(FnPtr, AST);

impl FromStr for SnapshotFn {
    type Err = RhaiErrorKinds;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let engine = Engine::new();
        let ast = engine.compile(s)?;
        let func = engine.eval_ast::<FnPtr>(&ast)?;
        Ok(Self(func, ast))
    }
}

impl SnapshotFn {
    fn call<T: Clone + Send + Sync + 'static>(
        &self,
        sn: &SnapshotFile,
    ) -> Result<T, Box<dyn Error>> {
        let engine = Engine::new();
        let sn: Dynamic = to_dynamic(sn)?;
        Ok(self.0.call::<T>(&engine, &self.1, (sn,))?)
    }
}

#[serde_as]
#[derive(Clone, Default, Debug, Deserialize, merge::Merge, clap::Parser)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct SnapshotFilter {
    /// Hostname to filter (can be specified multiple times)
    #[clap(long, global = true, value_name = "HOSTNAME")]
    #[merge(strategy=merge::vec::overwrite_empty)]
    filter_host: Vec<String>,

    /// Label to filter (can be specified multiple times)
    #[clap(long, global = true, value_name = "LABEL")]
    #[merge(strategy=merge::vec::overwrite_empty)]
    filter_label: Vec<String>,

    /// Path list to filter (can be specified multiple times)
    #[clap(long, global = true, value_name = "PATH[,PATH,..]")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[merge(strategy=merge::vec::overwrite_empty)]
    filter_paths: Vec<StringList>,

    /// Tag list to filter (can be specified multiple times)
    #[clap(long, global = true, value_name = "TAG[,TAG,..]")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[merge(strategy=merge::vec::overwrite_empty)]
    filter_tags: Vec<StringList>,

    /// Function to filter snapshots
    #[clap(long, global = true, value_name = "FUNC")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    filter_fn: Option<SnapshotFn>,
}

impl SnapshotFilter {
    #[must_use]
    pub fn matches(&self, snapshot: &SnapshotFile) -> bool {
        if let Some(filter_fn) = &self.filter_fn {
            match filter_fn.call::<bool>(snapshot) {
                Ok(result) => {
                    if !result {
                        return false;
                    }
                }
                Err(err) => {
                    warn!(
                        "Error evaluating filter-fn for snapshot {}: {err}",
                        snapshot.id
                    );
                }
            }
        }

        snapshot.paths.matches(&self.filter_paths)
            && snapshot.tags.matches(&self.filter_tags)
            && (self.filter_host.is_empty() || self.filter_host.contains(&snapshot.hostname))
            && (self.filter_label.is_empty() || self.filter_label.contains(&snapshot.label))
    }
}
