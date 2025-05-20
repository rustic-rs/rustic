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

use rustic_rs::application::RUSTIC_APP;

/// Boot Rustic
fn main() {
    abscissa_core::boot(&RUSTIC_APP);
}
