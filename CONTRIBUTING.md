# Contributing to `rustic`

Thank you for your interest in contributing to `rustic`!

We appreciate your help in making this project better.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [How to Contribute](#how-to-contribute)
  - [Reporting Bugs](#reporting-bugs)
  - [Suggesting Enhancements](#suggesting-enhancements)
  - [Code Style and Formatting](#code-style-and-formatting)
  - [Testing](#testing)
  - [Submitting Pull Requests](#submitting-pull-requests)
- [Development Setup](#development-setup)
- [License](#license)

## Code of Conduct

Please review and abide by the [Code of Conduct](CODE_OF_CONDUCT.md) when contributing to this project.

## How to Contribute

### Reporting Bugs

If you find a bug, please open an issue on GitHub and provide as much detail as possible. Include steps to reproduce the bug and the expected behavior.

### Suggesting Enhancements

If you have an idea for an enhancement or a new feature, we'd love to hear it! Open an issue on GitHub and describe your suggestion in detail.

### Code Style and Formatting

We follow the Rust community's best practices for code style and formatting. Before submitting code changes, please ensure your code adheres to these guidelines:

- Use `rustfmt` to format your code. You can run it with the following command:

  ```bash
  cargo fmt
  ```

- Write clear and concise code with meaningful variable and function names.

### Testing

We value code quality and maintainability. If you are adding new features or making changes, please include relevant unit tests. Run the test suite with:

```bash
cargo test
```

Make sure all tests pass before submitting your changes.

### Submitting Pull Requests

To contribute code changes, follow these steps:

1. **Fork** the repository.

2. **Create** a new branch with a descriptive name:

   ```bash
   git checkout -b feature/your-feature-name
   ```

3. **Commit** your changes:

   ```bash
   git commit -m "Add your meaningful commit message here"
   ```

4. **Push** your branch to your forked repository:

   ```bash
   git push origin feature/your-feature-name
   ```

5. **Open** a Pull Request (PR) to our repository. Please include a detailed description of the changes and reference any related issues.

Once your PR is submitted, it will be reviewed by the maintainers. We may suggest changes or ask for clarifications before merging.

## Development Setup

If you want to set up a local development environment, follow the steps outlined in the [README.md](README.md) file.

## License

By contributing to `rustic` or any crates contained in this repository, you agree that your contributions will be licensed under either of:

- [Apache License, Version 2.0](./LICENSE-APACHE)
- [MIT license](./LICENSE-MIT)

at your option. Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
