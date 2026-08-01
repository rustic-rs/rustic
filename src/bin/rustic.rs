//! Main entry point for Rustic

#![deny(warnings, missing_docs, trivial_casts, unused_qualifications)]
#![allow(unsafe_code)]

#[cfg(all(feature = "mimalloc", feature = "jemallocator"))]
compile_error!(
    "feature \"mimalloc\" and feature \"jemallocator\" cannot be enabled at the same time. Please disable one of them."
);

#[cfg(feature = "mimalloc")]
use mimalloc::MiMalloc;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

// jemallocator-global registers the #[global_allocator] from within the crate, so it has
// to be referenced here: an unused dependency is not linked into the binary and the
// allocator would silently stay the default one.
#[cfg(feature = "jemallocator")]
use jemallocator_global as _;

use rustic_rs::application::RUSTIC_APP;

/// Boot Rustic
fn main() {
    abscissa_core::boot(&RUSTIC_APP);
}
