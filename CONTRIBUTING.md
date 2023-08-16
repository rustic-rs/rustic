# Contributing to `rustic`

Thank you for your interest in contributing to `rustic`!

We appreciate your help in making this project better.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [How to Contribute](#how-to-contribute)
  - [Reporting Bugs](#reporting-bugs)
  - [Issue and Pull Request Labels](#issue-and-pull-request-labels)
  - [Suggesting Enhancements](#suggesting-enhancements)
  - [Code Style and Formatting](#code-style-and-formatting)
  - [Testing](#testing)
  - [Submitting Pull Requests](#submitting-pull-requests)
    - [Rebasing and other workflows](#rebasing-and-other-workflows)
- [Development Setup](#development-setup)
- [License](#license)

## Code of Conduct

Please review and abide by the general Rust Community [Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct) when contributing to this project. In the future, we might create our own Code of Conduct and supplement it at this location.

## How to Contribute

### Reporting Bugs

If you find a bug, please open an [issue on GitHub](https://github.com/rustic-rs/rustic/issues/new/choose) and provide as much detail as possible. Include steps to reproduce the bug and the expected behavior.

### Issue and Pull Request labels

Our Issues and Pull Request labels follow the official Rust style:

```text
A - Area
C - Category
D - Diagnostic
E - Call for participation
F - Feature
I - Issue e.g. I-crash
M - Meta
O - Operating systems
P - priorities e.g. P-{low, medium, high, critical}
PG - Project Group
perf - Performance
S - Status e.g. S-{blocked, experimental, inactive}
T - Team relevancy
WG - Working group
```

### Suggesting Enhancements

If you have an idea for an enhancement or a new feature, we'd love to hear it! Open an [issue on GitHub](https://github.com/rustic-rs/rustic/issues/new/choose) and describe your suggestion in detail.

### Code Style and Formatting

We follow the Rust community's best practices for code style and formatting. Before submitting code changes, please ensure your code adheres to these guidelines:

- Use `rustfmt` to format your code. You can run it with the following command:

  ```bash
  cargo fmt --all
  ```

- Write clear and concise code with meaningful, self-describing variable and function names. This tells the reader **what** the code does.

- Write clear and consise comments to tell the reader **why** you chose to implement it that way and **which** problem it solves.

### Testing

We value code quality and maintainability. If you are adding new features or making changes, please include relevant unit tests. Run the test suite with:

```bash
cargo test --workspace
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

#### `Release early and often!` also applies to pull requests

Consider drafting a Pull request early in the development process, so we can follow your progress and can give early feedback.

Once your PR is submitted, it will be reviewed by the maintainers. We may suggest changes or ask for clarifications before merging.

#### IMPORTANT NOTE

Please don't force push commits in your branch, in order to keep commit history and make it easier for us to see changes between reviews.

Make sure to Allow edits of maintainers (under the text box) in the PR so people can actually collaborate on things or fix smaller issues themselves.

#### Rebasing and other workflows

(taken from: [openage on rebasing](https://github.com/SFTtech/openage/blob/master/doc/contributing.md#rebasing))

**Rebasing** is 'moving' your commits to a different parent commit.

In other words: *Cut off* your branch from its tree, and *attach it* somewhere else.

There's two main applications:

- If you based your work on a older master (so old that stuff can't be automatically merged),
  you can rebase to move your commits to the current [upstream](https://help.github.com/articles/fork-a-repo/) master:

```bash
# update the upstream remote to receive new commits
git fetch upstream

# be on your feature branch (you probably are)
git checkout my-awesome-feature

# make backup (you never know, you know?)
git branch my-awesome-feature-backup

# rebase: put your commits on top of upstream's master
git rebase -m upstream/master
```

- If you want to fix an older commit of yours, or merge several commits into a single one (**squash** them), rebase interactively.
  We ***don't*** want to have a commit history like this:

  - `add stuff`
  - `fix typo in stuff`
  - `fix compilation`
  - `change stuff a bit`
  - and so on...

##### `rebase` in practice

`git log --graph --oneline` shows your commit history as graph.
To make some changes in that graph, you do an **interactive rebase**:

```sh
git rebase -i -m upstream/master
```

With this command, your new "base" is `upstream/master` and you can
then change any of your branch's commits.

`-i` will open an interactive editor where you can choose actions for each individual commit:

- re-order commits
- drop commits by deleting their line
- squash/fixup ("meld") your commits
- reword a commit message
- stop rebasing at a commit to edit (`--amend`) it manually

Just follow the messages on screen.

##### Changing commits with `amend` and `fixup`

There's also `git commit --amend` which is a "mini-rebase" that modifies just the last commit with your current changes by `git add`.
It just skips the creation of a new commit and instead melds the changes into the last one you made.

If you want to update a single commit in the range `[upstream/master, current HEAD]` which is not the last commit:

- `edit stuff you wanna change in some previous commit`
- `git add changed_stuff`
- `git commit --fixup $hash_of_commit_to_be_fixed`
- `git rebase --autosquash -i -m upstream/master`

##### Pushing changes

After you have rebased stuff (["rewritten history"](https://www.youtube.com/watch?v=9lXuZHkOoH8)) that had already been pushed,
git will not accept your pushes because they're not simple fast-forwards:

- The commit contents and the parent commit have changed as you updated the commit, therefore the commit hash changed, too.
  - If somebody used those commits, they will keep a copy
    and have a hard time updating to your updated version (because they "use" the old hashes).
  - Update your pull request branch with your re-written history!

- **force push** is the standard way of overwriting your development work with the fixed and mergeable version of your contribution!
  - Why? You changed the commits, so you want the old ones to be deleted!

  You can use any of:
  - `git push origin +my-awesome-feature`
  - `git push origin -f my-awesome-feature`
  - `git push origin --force my-awesome-feature`

Some extra tutorials on `git rebase`:

- [Atlassian's Git Tutorial](https://www.atlassian.com/git/tutorials/rewriting-history/)
- [Pro Git book](http://git-scm.com/book)
- `man git-rebase`

## Development Setup

If you want to set up a local development environment, follow the steps in the [development guide](/docs/dev/development_guide.md) file - which is currently being worked on.

## License

By contributing to `rustic` or any crates contained in this repository, you agree that your contributions will be licensed under:

- [Apache License, Version 2.0](./LICENSE-APACHE)
- [MIT license](./LICENSE-MIT).

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
