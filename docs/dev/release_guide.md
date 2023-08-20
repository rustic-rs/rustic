# Release Guide

Work in progress ...

## `rustic-rs`

TODO

## `rustic_core`

1. Public API

   Run:

   `cargo test --test public_api -p rustic_core -- --ignored`

   to check if the public API has changed and the version number needs to be
   bumped.

   Helpful advices are also given by the
   [Cargo reference](https://doc.rust-lang.org/cargo/reference/semver.html).

1. Version number

   Depending of the outcome of the Public API check, bump the corresponding
   version number in `rustic_core/Cargo.toml`.

1. Use the `release`-Branch

   Push the changes to a `release/vX.Y.Z` branch in the repository

... TODO! ...

1. Publishing to crates.io

   Run:

   `cargo publish --manifest-path rustic_core/Cargo.toml`

TODO: Include `cargo smart-release` into the release process.

TODO:
<https://github.com/cargo-bins/cargo-binstall/blob/main/.github/workflows/release-pr.yml>
for implementing a release workflow based on a PR.
