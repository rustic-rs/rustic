/*!
rustic

Application based on the [Abscissa] framework.

[Abscissa]: https://github.com/iqlusioninc/abscissa
*/

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
#![allow(
    clippy::module_name_repetitions,
    clippy::redundant_pub_crate,
    clippy::missing_const_for_fn
)]

pub mod application;
pub(crate) mod commands;
pub(crate) mod config;
pub(crate) mod error;
pub(crate) mod filtering;
pub(crate) mod helpers;

// rustic_cli Public API

/// Abscissa core prelude
pub use abscissa_core::prelude::*;

/// Application state
pub use crate::application::RUSTIC_APP;

/// Rustic config
pub use crate::config::RusticConfig;

/// Completions
pub use crate::commands::completions::generate_completion;
