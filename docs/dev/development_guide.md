# Development guide

Work in progress ...

## `cargo xtask`

We utilize `cargo xtask` to provide additional functionality for the development
process.

### Usage

Currently it supports the following functionalities:

| Command                    | Description                                                       |
| -------------------------- | ----------------------------------------------------------------- |
| `cargo xtask help`         | Show an overview over all or the help of the given subcommand(s). |
| `cargo xtask bloat-deps`   | Show biggest crates in release build using cargo-bloat.           |
| `cargo xtask bloat-time`   | Show longest times taken in release build using cargo-bloat.      |
| `cargo xtask coverage`     | Generate code coverage report.                                    |
| `cargo xtask install-deps` | Install dependencies for the development process.                 |
| `cargo xtask timings`      | Show longest times taken in release build using cargo-bloat.      |

## Justfile

We utilize `just` to provide additional functionality for the development
process.

### Installation

Install `just` with:

```bash
cargo install just
```

or by using [`scoop`](https://scoop.sh/):

```bash
scoop install just
```

### Usage

Currently it supports the following functionalities:

| Command             | Description                                                                      |
| ------------------- | -------------------------------------------------------------------------------- |
| `just build`        | Builds the library.                                                              |
| `just check`        | Checks the library for syntax and HIR errors.                                    |
| `just ci`           | Runs all of the recipes necessary for pre-publish.                               |
| `just clean`        | Removes all build artifacts.                                                     |
| `just dev`          | Runs the development routines.                                                   |
| `just doc`          | Opens the crate documentation.                                                   |
| `just format`       | Runs the formatter on all Rust files.                                            |
| `just lint`         | Runs the linter.                                                                 |
| `just loop`         | Continually runs some recipe from this file.                                     |
| `just miri *ARGS`   | Looks for undefined behavior in the (non-doc) tests defined by `*ARGS`.          |
| `just natest *ARGS` | Runs a test defined by `*ARGS` with nextest.                                     |
| `just ntest`        | Runs the whole test suite with nextest.                                          |
| `just nitest`       | Runs only the ignored tests with nextest.                                        |
| `just package`      | Packages the crate in preparation for publishing on crates.io.                   |
| `just publish`      | Publishes the crate to crates.io.                                                |
| `just test`         | Runs the test suites.                                                            |
| `just tpa`          | Runs a test to check if the public api of rustic_core has been changed.          |
| `just coverage`     | Generate code coverage report.                                                   |
| `just inv-ft *ARGS` | List the inverse dependencies as in which feature enables a given crate `*ARGS`. |
