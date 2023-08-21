//! Main entry point for Rustic

#![deny(warnings, missing_docs, trivial_casts, unused_qualifications)]
#![allow(unsafe_code)]

use rustic_rs::application::RUSTIC_APP;

/// Boot Rustic
fn main() {
    // TODO: this needs to be handled?
    // this is a workaround until unix_sigpipe (https://github.com/rust-lang/rust/issues/97889) is available.
    // See also https://github.com/rust-lang/rust/issues/46016
    #[cfg(not(windows))]
    #[allow(unsafe_code)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    abscissa_core::boot(&RUSTIC_APP);
}
