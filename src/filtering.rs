use crate::error::RhaiErrorKinds;

use log::warn;
use rustic_core::{repofile::SnapshotFile, StringList};
use std::{error::Error, str::FromStr};

use cached::proc_macro::cached;
use rhai::{serde::to_dynamic, Dynamic, Engine, FnPtr, AST};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

/// A function to filter snapshots
///
/// The function is called with a [`SnapshotFile`] and must return a boolean.
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

#[cached(key = "String", convert = r#"{ s.to_string() }"#, size = 1)]
fn string_to_fn(s: &str) -> Option<SnapshotFn> {
    match SnapshotFn::from_str(s) {
        Ok(filter_fn) => Some(filter_fn),
        Err(err) => {
            warn!("Error evaluating filter-fn {s}: {err}",);
            None
        }
    }
}

impl SnapshotFn {
    /// Call the function with a [`SnapshotFile`]
    ///
    /// The function must return a boolean.
    ///
    /// # Errors
    ///
    // TODO!: add errors!
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
#[derive(Clone, Default, Debug, Serialize, Deserialize, merge::Merge, clap::Parser)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct SnapshotFilter {
    /// Hostname to filter (can be specified multiple times)
    #[clap(long = "filter-host", global = true, value_name = "HOSTNAME")]
    #[merge(strategy=merge::vec::overwrite_empty)]
    filter_hosts: Vec<String>,

    /// Label to filter (can be specified multiple times)
    #[clap(long = "filter-label", global = true, value_name = "LABEL")]
    #[merge(strategy=merge::vec::overwrite_empty)]
    filter_labels: Vec<String>,

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
    filter_fn: Option<String>,
}

impl SnapshotFilter {
    /// Check if a [`SnapshotFile`] matches the filter
    ///
    /// # Arguments
    ///
    /// * `snapshot` - The snapshot to check
    ///
    /// # Returns
    ///
    /// `true` if the snapshot matches the filter, `false` otherwise
    #[must_use]
    pub fn matches(&self, snapshot: &SnapshotFile) -> bool {
        if let Some(filter_fn) = &self.filter_fn {
            if let Some(func) = string_to_fn(filter_fn) {
                match func.call::<bool>(snapshot) {
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
        }

        snapshot.paths.matches(&self.filter_paths)
            && snapshot.tags.matches(&self.filter_tags)
            && (self.filter_hosts.is_empty() || self.filter_hosts.contains(&snapshot.hostname))
            && (self.filter_labels.is_empty() || self.filter_labels.contains(&snapshot.label))
    }
}
