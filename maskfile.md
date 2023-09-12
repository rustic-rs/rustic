# Tasks

Development tasks for rustic.

You can run this file with [mask](https://github.com/jacobdeichert/mask/).

Install `mask` with `cargo install mask`.

## check

> Checks the library for syntax and HIR errors.

Bash:

```powershell
cargo check --no-default-features \
    && cargo check --all-features
```

PowerShell:

```powershell
[Diagnostics.Process]::Start("cargo", "check --no-default-features").WaitForExit()
[Diagnostics.Process]::Start("cargo", "cargo check --all-features").WaitForExit()
```

## ci

> Continually runs the development routines.

Bash:

```powershell
mask loop dev
```

PowerShell:

```powershell
[Diagnostics.Process]::Start("mask", "loop dev").WaitForExit()
```

## clean

> Removes all build artifacts.

Bash:

```powershell
cargo clean
```

PowerShell:

```powershell
[Diagnostics.Process]::Start("cargo", "clean").WaitForExit()
```

## dev

> Runs the development routines

Bash:

```powershell
$MASK format \
    && $MASK lint \
    && $MASK test \
    && $MASK doc
```

PowerShell:

```powershell
[Diagnostics.Process]::Start("mask", "format").WaitForExit()
[Diagnostics.Process]::Start("mask", "lint").WaitForExit()
[Diagnostics.Process]::Start("mask", "test").WaitForExit()
[Diagnostics.Process]::Start("mask", "doc").WaitForExit()
```

## doc (crate)

> Opens the crate documentation

Bash:

```powershell
cargo doc --all-features --no-deps --open $crate
```

PowerShell:

```powershell
[Diagnostics.Process]::Start("cargo", "doc --all-features --no-deps --open $crate").WaitForExit()
```

## format

> Run formatters on the repository.

### format cargo

> Runs the formatter on all Rust files.

Bash:

```powershell
cargo fmt --all
```

PowerShell:

```powershell
[Diagnostics.Process]::Start("cargo", "fmt --all").WaitForExit()
```

### format dprint

> Runs the formatter on md, json, and toml files

Bash:

```powershell
dprint fmt
```

Powershell:

PowerShell:

```powershell
[Diagnostics.Process]::Start("dprint", "fmt").WaitForExit()
```

### format all

> Runs all the formatters.

Bash:

```powershell
$MASK format cargo \
    && $MASK format dprint
```

PowerShell:

```powershell
[Diagnostics.Process]::Start("mask", "format cargo").WaitForExit()
[Diagnostics.Process]::Start("mask", "format dprint").WaitForExit()
```

## inverse-deps (crate)

> Lists all crates that depend on the given crate

Bash:

```powershell
cargo tree -e features -i $crate
```

PowerShell:

```powershell
[Diagnostics.Process]::Start("cargo", "tree -e features -i $crate").WaitForExit()
```

## lint

> Runs the linter

Bash:

```powershell
$MASK check \
    && cargo clippy --no-default-features -- -D warnings \
    && cargo clippy --all-features -- -D warnings
```

PowerShell:

```powershell
[Diagnostics.Process]::Start("mask", "check").WaitForExit()
[Diagnostics.Process]::Start("cargo", "clippy --no-default-features -- -D warnings").WaitForExit()
[Diagnostics.Process]::Start("cargo", "clippy --all-features -- -D warnings").WaitForExit()
```

## loop (action)

> Continually runs some recipe from this file.

Bash:

```powershell
watchexec -w src -- "$MASK $action"
```

PowerShell:

```powershell
[Diagnostics.Process]::Start("watchexec", "-w src -- $MASK $action).WaitForExit()
```

## miri (tests)

> Looks for undefined behavior in the (non-doc) test suite.

**NOTE**: This requires the nightly toolchain.

Bash:

```powershell
cargo +nightly miri test --all-features -q --lib --tests $tests
```

PowerShell:

```powershell
[Diagnostics.Process]::Start("cargo", "+nightly miri test --all-features -q --lib --tests $tests").WaitForExit()
```

## nextest

> Runs the whole test suite with nextest.

### nextest ignored

> Runs the whole test suite with nextest on the workspace, including ignored
> tests.

Bash:

```powershell
cargo nextest run -r --all-features --workspace -- --ignored
```

PowerShell:

```powershell
[Diagnostics.Process]::Start("cargo", "nextest run -r --all-features --workspace -- --ignored").WaitForExit()
```

### nextest ws

> Runs the whole test suite with nextest on the workspace.

Bash:

```powershell
cargo nextest run -r --all-features --workspace
```

PowerShell:

```powershell
[Diagnostics.Process]::Start("cargo", "nextest run -r --all-features --workspace").WaitForExit()
```

### nextest test

> Runs a single test with nextest.

- test
  - flags: -t, --test
  - type: string
  - desc: Only run the specified test target
  - required

Bash:

```powershell
cargo nextest run -r --all-features -E "test($test)"
```

PowerShell:

```powershell
[Diagnostics.Process]::Start("cargo", "nextest run -r --all-features -E 'test($test)'").WaitForExit()
```

## pr

> Prepare a Contribution/Pull request and run necessary checks and lints

Bash:

```powershell
$MASK fmt \
    && $MASK test \
    && $MASK lint
```

PowerShell:

```powershell
[Diagnostics.Process]::Start("mask", "fmt").WaitForExit()
[Diagnostics.Process]::Start("mask", "test").WaitForExit()
[Diagnostics.Process]::Start("mask", "lint").WaitForExit()
```

## public-api

> Runs a test to check if the public api of `rustic_core` has been changed

### public-api update

> Updates the test files for the public api test

Bash:

```powershell
export UPDATE_EXPECT=1
cargo test --test public_api -p rustic_core -- --ignored
```

PowerShell:

```powershell
$env:UPDATE_EXPECT=1
[Diagnostics.Process]::Start("cargo", "test --test public_api -p rustic_core -- --ignored").WaitForExit()
```

### public-api test

> Runs the public api test

Bash:

```powershell
cargo test --test public_api -p rustic_core -- --ignored
```

PowerShell:

```powershell
[Diagnostics.Process]::Start("cargo", "test --test public_api -p rustic_core -- --ignored").WaitForExit()
```

## test

> Runs the test suites.

Bash:

```powershell
$MASK check \
    && $MASK lint
    && cargo test --all-features
```

PowerShell:

```powershell
[Diagnostics.Process]::Start("mask", "check").WaitForExit()
[Diagnostics.Process]::Start("mask", "lint").WaitForExit()
[Diagnostics.Process]::Start("cargo", "test --all-features").WaitForExit()
```
