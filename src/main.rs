// TODO: add
//    missing_docs,
//    unused_results,
//    trivial_casts??
#![warn(
    bad_style,
    const_err,
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
    unsafe_code,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications,
    unconditional_recursion,
    unused,
    unused_allocation,
    unused_comparisons,
    unused_parens,
    while_true
)]

use anyhow::Result;

mod archiver;
mod backend;
mod blob;
mod chunker;
mod commands;
mod crypto;
mod id;
mod index;
mod repo;

fn main() -> Result<()> {
    commands::execute()
}
