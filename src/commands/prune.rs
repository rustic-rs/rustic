//! `prune` subcommand

use crate::{
    Application, RUSTIC_APP, helpers::bytes_size_to_string, repository::OpenRepo, status_err,
};
use abscissa_core::{Command, Runnable, Shutdown};
use log::{debug, info};

use anyhow::Result;

use rustic_core::{PruneOptions, PruneStats, repofile::BlobType};

/// `prune` subcommand
#[allow(clippy::struct_excessive_bools)]
#[derive(clap::Parser, Command, Debug, Clone)]
pub(crate) struct PruneCmd {
    /// Prune options
    #[clap(flatten)]
    pub(crate) opts: PruneOptions,
}

/// Prune-plan values that can be reported after a successful prune run.
///
/// The core prune API exposes exact blob counts and the compressed/encrypted blob lengths for
/// the plan. It intentionally does not expose raw, uncompressed lengths for the selected blobs,
/// so callers must not derive raw-byte metrics from these values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PruneMetrics {
    /// Unmarked packs containing no referenced blobs, as identified by the prune plan.
    ///
    /// A `--keep-pack` setting can retain a young pack even when it is counted here. Without
    /// `--instant-delete`, selected packs are marked for later deletion rather than physically
    /// removed immediately.
    pub(crate) packs_unreferenced: u64,
    /// Packs selected for rewriting by the prune plan.
    pub(crate) packs_rewritten: u64,
    /// Unmarked packs selected to remain untouched by the prune plan.
    pub(crate) packs_kept: u64,
    /// Data blobs removed from the active index by selected repack or removal actions.
    pub(crate) data_blobs_removed: u64,
    /// Tree blobs removed from the active index by selected repack or removal actions.
    pub(crate) tree_blobs_removed: u64,
    /// Compressed/encrypted data-blob bytes removed from the active index by the plan.
    pub(crate) data_removed_packed: u64,
    /// Compressed/encrypted tree-blob bytes removed from the active index by the plan.
    pub(crate) tree_removed_packed: u64,
}

impl PruneMetrics {
    fn from_stats(stats: &PruneStats) -> Self {
        let data_blobs = stats.blobs[BlobType::Data];
        let tree_blobs = stats.blobs[BlobType::Tree];
        let data_size = stats.size[BlobType::Data];
        let tree_size = stats.size[BlobType::Tree];

        Self {
            packs_unreferenced: stats.packs.unused,
            packs_rewritten: stats.packs.repack,
            packs_kept: stats.packs.keep,
            data_blobs_removed: data_blobs.repackrm + data_blobs.remove,
            tree_blobs_removed: tree_blobs.repackrm + tree_blobs.remove,
            data_removed_packed: data_size.repackrm + data_size.remove,
            tree_removed_packed: tree_size.repackrm + tree_size.remove,
        }
    }
}

impl Runnable for PruneCmd {
    fn run(&self) {
        if let Err(err) = RUSTIC_APP
            .config()
            .repository
            .run_open(|repo| self.inner_run(&repo).map(|_| ()))
        {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl PruneCmd {
    pub(crate) fn inner_run(&self, repo: &OpenRepo) -> Result<PruneMetrics> {
        let config = RUSTIC_APP.config();

        let prune_plan = repo.prune_plan(&self.opts)?;

        print_stats(&prune_plan.stats);
        let metrics = PruneMetrics::from_stats(&prune_plan.stats);

        let dry_run = config.global.dry_run;
        if dry_run && config.global.dry_run_warmup {
            repo.warm_up(prune_plan.repack_packs().into_iter())?;
        } else if !config.global.dry_run_warmup {
            debug!("Ignoring --dry-run-warmup works only in combination with --dry-run");
        }
        if !dry_run {
            repo.prune(&self.opts, prune_plan)?;
        }

        Ok(metrics)
    }
}

/// Print statistics about the prune operation
///
/// # Arguments
///
/// * `stats` - Statistics about the prune operation
#[allow(clippy::cast_precision_loss)]
fn print_stats(stats: &PruneStats) {
    let pack_stat = &stats.packs;
    let blob_stat = stats.blobs_sum();
    let size_stat = stats.size_sum();

    debug!("statistics:");
    debug!("{:#?}", stats.debug);

    debug!(
        "used:   {:>10} blobs, {:>10}",
        blob_stat.used,
        bytes_size_to_string(size_stat.used)
    );

    debug!(
        "unused: {:>10} blobs, {:>10}",
        blob_stat.unused,
        bytes_size_to_string(size_stat.unused)
    );
    debug!(
        "total:  {:>10} blobs, {:>10}",
        blob_stat.total(),
        bytes_size_to_string(size_stat.total())
    );

    info!(
        "to repack: {:>10} packs, {:>10} blobs, {:>10}",
        pack_stat.repack,
        blob_stat.repack,
        bytes_size_to_string(size_stat.repack)
    );
    info!(
        "this removes:                {:>10} blobs, {:>10}",
        blob_stat.repackrm,
        bytes_size_to_string(size_stat.repackrm)
    );
    info!(
        "to delete: {:>10} packs, {:>10} blobs, {:>10}",
        pack_stat.unused,
        blob_stat.remove,
        bytes_size_to_string(size_stat.remove)
    );
    if stats.packs_unref > 0 {
        info!(
            "unindexed: {:>10} packs,         ?? blobs, {:>10}",
            stats.packs_unref,
            bytes_size_to_string(stats.size_unref)
        );
    }

    info!(
        "total prune:                 {:>10} blobs, {:>10}",
        blob_stat.repackrm + blob_stat.remove,
        bytes_size_to_string(size_stat.repackrm + size_stat.remove + stats.size_unref)
    );
    info!(
        "remaining:                   {:>10} blobs, {:>10}",
        blob_stat.total_after_prune(),
        bytes_size_to_string(size_stat.total_after_prune())
    );
    info!(
        "unused size after prune: {:>10} ({:.2}% of remaining size)",
        bytes_size_to_string(size_stat.unused_after_prune()),
        size_stat.unused_after_prune() as f64 / size_stat.total_after_prune() as f64 * 100.0
    );

    info!(
        "packs marked for deletion: {:>10}, {:>10}",
        stats.packs_to_delete.total(),
        bytes_size_to_string(stats.size_to_delete.total()),
    );
    info!(
        " - complete deletion:      {:>10}, {:>10}",
        stats.packs_to_delete.remove,
        bytes_size_to_string(stats.size_to_delete.remove),
    );
    info!(
        " - keep marked:            {:>10}, {:>10}",
        stats.packs_to_delete.keep,
        bytes_size_to_string(stats.size_to_delete.keep),
    );
    info!(
        " - recover:                {:>10}, {:>10}",
        stats.packs_to_delete.recover,
        bytes_size_to_string(stats.size_to_delete.recover),
    );

    debug!(
        "index files to rebuild: {} / {}",
        stats.index_files_rebuild, stats.index_files
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_metrics_use_the_selected_plan_actions() {
        let mut stats = PruneStats::default();
        stats.packs.unused = 3;
        stats.packs.repack = 2;
        stats.packs.keep = 7;

        stats.blobs[BlobType::Data].repackrm = 11;
        stats.blobs[BlobType::Data].remove = 13;
        stats.blobs[BlobType::Tree].repackrm = 17;
        stats.blobs[BlobType::Tree].remove = 19;

        stats.size[BlobType::Data].repackrm = 23;
        stats.size[BlobType::Data].remove = 29;
        stats.size[BlobType::Tree].repackrm = 31;
        stats.size[BlobType::Tree].remove = 37;

        assert_eq!(
            PruneMetrics::from_stats(&stats),
            PruneMetrics {
                packs_unreferenced: 3,
                packs_rewritten: 2,
                packs_kept: 7,
                data_blobs_removed: 24,
                tree_blobs_removed: 36,
                data_removed_packed: 52,
                tree_removed_packed: 68,
            }
        );
    }
}
