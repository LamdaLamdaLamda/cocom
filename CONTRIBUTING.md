# Contributing to Cocom

Thanks for your interest in contributing! This document outlines the process for contributing to the project.

## Table of Contents

- [Getting Started](#getting-started)
- [How to Contribute](#how-to-contribute)
- [Coding Guidelines](#coding-guidelines)
- [Commit Message Conventions](#commit-message-conventions)
- [Pull Request Process](#pull-request-process)
- [Reporting Bugs](#reporting-bugs)
- [Suggesting Features](#suggesting-features)

## Getting Started

1. Fork the repository
2. Clone your fork:
   ```bash
   git clone git@github.com:LamdaLamdaLamda/cocom.git
   cd cocom
   ```
3. Add the upstream repository as a remote:
   ```bash
   git remote add upstream git@github.com:LamdaLamdaLamda/cocom.git
   ```
4. Create a new branch for your change:
   ```bash
   git checkout -b feature/short-description
   ```

## How to Contribute

- **Bug fixes** — always welcome, small and focused PRs are easiest to review
- **Features** — please open an issue first to discuss scope and approach before investing significant time
- **Documentation** — typos, clarifications, and examples are always appreciated
- **Tests** — improving test coverage is a great way to get familiar with the codebase

## Coding Guidelines

- Keep changes focused — one logical change per PR
- Match the existing code style (formatting follows `rustfmt` defaults; run `cargo fmt` before committing)
- Run `cargo clippy` and address warnings before opening a PR
- Add or update tests for any behavioral change
- Update documentation (README, inline comments) when behavior changes
- Avoid unrelated formatting-only changes mixed into functional PRs

## Commit Message Conventions

This project uses [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <short summary>

[optional body]

[optional footer]
```

Common types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`

Examples:

```
feat(client): add support for IPv6 bind addresses
fix(ntp): correct fraction rounding in Timestamp conversion
docs(readme): clarify installation steps
```

## Pull Request Process

1. Ensure your branch is up to date with `main`:
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```
2. Make sure all tests pass locally (`just test`)
3. Update `CHANGELOG.md` under the `Unreleased` section
4. Open a PR against `main` with a clear description of *what* and *why*
5. Link related issues (e.g. `Closes #123`)
6. Sign your commits if possible (`git commit -S`)
7. Address review feedback — a maintainer will merge once approved

PRs require at least one maintainer approval and a passing CI run before merging.

## Reporting Bugs

Please open an issue including:

- Expected vs. actual behavior
- Steps to reproduce
- Version / environment (OS, Rust version, etc.)
- Relevant logs or error output

## Suggesting Features

Open an issue describing:

- The problem you're trying to solve (not just the solution)
- Any alternatives you've considered
- Whether you're willing to implement it yourself

---

Questions? Open a [Discussion](../../discussions) or reach out via an issue.
