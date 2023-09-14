/*!
A library for deduplicated and encrypted backups, using repositories as specified in the [`restic repository design`](https://github.com/restic/restic/blob/master/doc/design.rst).

# Overview

This section gives a brief overview of the primary types in this crate:

TODO

# Example - initialize a repository, backup to it and get snapshots

```
    use rustic_core::{BackupOptions, ConfigOptions, KeyOptions, PathList, Repository, RepositoryOptions, SnapshotOptions};

    // Initialize the repository in a temporary dir
    let repo_dir = tempfile::tempdir().unwrap();
    let repo_opts = RepositoryOptions::default()
        .repository(repo_dir.path().to_str().unwrap())
        .password("test");
    let key_opts = KeyOptions::default();
    let config_opts = ConfigOptions::default();
    let _repo = Repository::new(&repo_opts).unwrap().init(&key_opts, &config_opts).unwrap();

    // We could have used _repo directly, but open the repository again to show how to open it...
    let repo = Repository::new(&repo_opts).unwrap().open().unwrap();

    // Get all snapshots from the repository
    let snaps = repo.get_all_snapshots().unwrap();
    // Should be zero, as the repository has just been initialized
    assert_eq!(snaps.len(), 0);

    // Turn repository state to indexed (for backup):
    let repo = repo.to_indexed_ids().unwrap();

    // Pre-define the snapshot-to-backup
    let snap = SnapshotOptions::default()
        .add_tags("tag1,tag2").unwrap()
        .to_snapshot().unwrap();

    // Specify backup options and source
    let backup_opts = BackupOptions::default();
    let source = PathList::from_string("src").unwrap().sanitize().unwrap();

    // run the backup and return the snapshot pointing to the backup'ed data.
    let snap = repo.backup(&backup_opts, source, snap).unwrap();
    // assert_eq!(&snap.paths, ["src"]);

    // Get all snapshots from the repository
    let snaps = repo.get_all_snapshots().unwrap();
    // Should now be 1, we just created a snapshot
    assert_eq!(snaps.len(), 1);

    assert_eq!(snaps[0], snap);
```

# Lower level APIs

TODO

# Crate features

This crate exposes a few features for controlling dependency usage.

*   **cli** -
    Enables support for CLI features by enabling `merg` and `clap` features.
*   **merge** -
    Enables support for merging multiple values into one, which enables the `merge`
    dependency. This is needed for parsing commandline arguments and merging them
    into one (e.g. config). This feature is disabled by default.
*   **clap** -
    Enables a dependency on the `clap` and `clap_complete` crate and enables
    parsing from the commandline. This feature is disabled by default.
*/

#![allow(dead_code)]
#![forbid(unsafe_code)]
#![warn(
    // TODO: frequently check
    // unreachable_pub,
    // TODO: Activate if you're feeling like fixing stuff 
    // clippy::pedantic,
    // clippy::correctness,
    // clippy::suspicious,
    // clippy::complexity,
    // clippy::perf,
    missing_docs,
    rust_2018_idioms,
    trivial_casts,
    unused_lifetimes,
    unused_qualifications,
    clippy::nursery,
    bad_style,
    dead_code,
    improper_ctypes,
    missing_copy_implementations,
    missing_debug_implementations,
    non_shorthand_field_patterns,
    no_mangle_generic_items,
    overflowing_literals,
    path_statements,
    patterns_in_fns_without_body,
    trivial_numeric_casts,
    unused_results,
    trivial_casts,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications,
    unconditional_recursion,
    unused,
    unused_allocation,
    unused_comparisons,
    unused_parens,
    while_true,
    clippy::cast_lossless,
    clippy::default_trait_access,
    clippy::doc_markdown,
    clippy::manual_string_new,
    clippy::match_same_arms,
    clippy::semicolon_if_nothing_returned,
    clippy::trivially_copy_pass_by_ref
)]
#![allow(clippy::module_name_repetitions, clippy::redundant_pub_crate)]
// TODO: Remove when Windows support landed
// mostly Windows-related functionality is missing `const`
// as it's only OK(()), but doesn't make it reasonable to
// have a breaking change in the future. They won't be const.
#![allow(clippy::missing_const_for_fn)]
// We run rustdoc with `--document-private-items` so we can document private items
#![allow(rustdoc::private_intra_doc_links)]

pub(crate) mod archiver;
pub(crate) mod backend;
pub(crate) mod blob;
pub(crate) mod cdc;
pub(crate) mod chunker;
pub(crate) mod commands;
pub(crate) mod crypto;
pub(crate) mod error;
pub(crate) mod id;
pub(crate) mod index;
pub(crate) mod progress;
/// Structs which are saved in JSON or binary format in the repository
pub mod repofile;
pub(crate) mod repository;

// rustic_core Public API
pub use crate::{
    backend::{
        decrypt::{compression_level_range, max_compression_level},
        ignore::{LocalSource, LocalSourceFilterOptions, LocalSourceSaveOptions},
        local::LocalDestination,
        node::last_modified_node,
        ReadSourceEntry,
    },
    blob::tree::TreeStreamerOptions as LsOptions,
    commands::{
        backup::{BackupOptions, ParentOptions},
        check::CheckOptions,
        config::ConfigOptions,
        copy::CopySnapshot,
        forget::{ForgetGroup, ForgetGroups, ForgetSnapshot, KeepOptions},
        key::KeyOptions,
        prune::{PruneOptions, PrunePlan, PruneStats},
        repair::{index::RepairIndexOptions, snapshots::RepairSnapshotsOptions},
        repoinfo::{BlobInfo, IndexInfos, PackInfo, RepoFileInfo, RepoFileInfos},
        restore::{FileDirStats, RestoreOptions, RestorePlan, RestoreStats},
    },
    error::{RusticError, RusticResult},
    id::{HexId, Id},
    progress::{NoProgress, NoProgressBars, Progress, ProgressBars},
    repofile::snapshotfile::{
        PathList, SnapshotGroup, SnapshotGroupCriterion, SnapshotOptions, StringList,
    },
    repository::{IndexedFull, OpenStatus, Repository, RepositoryOptions},
};
