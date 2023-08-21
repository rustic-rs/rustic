/*!
A library for deduplicated and encrypted backups, inspired by [`restic`](https://restic.net/).

# Overview

This section gives a brief overview of the primary types in this crate:

TODO

# Examples

TODO

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
    // unreachable_pub, // frequently check
    // TODO: Activate and create better docs
    // missing_docs,
    rust_2018_idioms,
    trivial_casts,
    unused_lifetimes,
    unused_qualifications,
    // TODO: Activate if you're feeling like fixing stuff 
    // clippy::pedantic,
    // clippy::correctness,
    // clippy::suspicious,
    // clippy::complexity,
    // clippy::perf,
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
    private_in_public,
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
pub(crate) mod repofile;
pub(crate) mod repository;

pub(crate) use crate::crypto::aespoly1305::Key;
// rustic_core Public API
pub use crate::{
    backend::{
        decrypt::{DecryptReadBackend, DecryptWriteBackend},
        ignore::{LocalSource, LocalSourceFilterOptions, LocalSourceSaveOptions},
        local::LocalDestination,
        node::{latest_node, Node, NodeType},
        stdin::StdinSource,
        FileType, ReadBackend, ReadSourceEntry, WriteBackend, ALL_FILE_TYPES,
    },
    blob::{
        packer::Packer,
        tree::{NodeStreamer, Tree, TreeStreamerOnce, TreeStreamerOptions},
        BlobType, BlobTypeMap, Initialize, Sum,
    },
    commands::{
        backup::{BackupOpts, ParentOpts},
        check::CheckOpts,
        config::ConfigOpts,
        copy::CopySnapshot,
        forget::{ForgetGroup, ForgetGroups, ForgetSnapshot, KeepOptions},
        key::KeyOpts,
        prune::{PruneOpts, PrunePlan, PruneStats},
        repair::{index::RepairIndexOptions, snapshots::RepairSnapshotsOptions},
        repoinfo::{BlobInfo, IndexInfos, PackInfo, RepoFileInfo, RepoFileInfos},
        restore::{FileDirStats, RestoreInfos, RestoreOpts, RestoreStats},
    },
    crypto::hasher::hash,
    error::{RusticError, RusticResult},
    id::Id,
    index::{indexer::Indexer, IndexBackend, IndexedBackend, ReadIndex},
    progress::{NoProgress, NoProgressBars, Progress, ProgressBars},
    repofile::{
        configfile::ConfigFile,
        indexfile::{IndexBlob, IndexFile, IndexPack},
        keyfile::KeyFile,
        packfile::{HeaderEntry, PackHeader, PackHeaderLength, PackHeaderRef},
        snapshotfile::{
            DeleteOption, PathList, SnapshotFile, SnapshotGroup, SnapshotGroupCriterion,
            SnapshotOptions, StringList,
        },
    },
    repository::{IndexedFull, Open, OpenStatus, Repository, RepositoryOptions},
};
