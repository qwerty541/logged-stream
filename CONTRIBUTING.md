# Contributing to logged-stream <!-- omit in toc -->

<details>
<summary>Table of contents</summary>

- [Description](#description)
- [Code of Conduct](#code-of-conduct)
- [Ways to Contribute](#ways-to-contribute)
- [Development Setup](#development-setup)
  - [Prerequisites](#prerequisites)
  - [Toolchain and MSRV](#toolchain-and-msrv)
  - [Building](#building)
  - [Testing](#testing)
  - [Linting \& Formatting](#linting--formatting)
  - [Documentation](#documentation)
  - [Examples](#examples)
  - [Benchmarks](#benchmarks)
- [Project Structure](#project-structure)
- [Performance \& Reliability](#performance--reliability)
- [Commit \& PR Etiquette](#commit--pr-etiquette)
- [Release Process](#release-process)
- [Security](#security)
- [License](#license)
</details>

## Description

Thanks for your interest in improving logged-stream! This document outlines how to propose changes, report issues, and develop locally. The project follows common practices used across the Rust crates community.

This document is intended to reflect the current state of the repository. If something doesn’t match what the code or CI actually does, please open an issue or a PR.

## Code of Conduct

This project adheres to a Code of Conduct. By participating, you agree to uphold it.

- See [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md)

## Ways to Contribute

- Report bugs or suggest improvements via GitHub Issues
- Improve documentation (README, rustdoc comments, examples)
- Add tests and benchmarks
- Implement features (check open issues or propose a new one)
- Help with issue triage (reproductions, platform checks, clarifying questions)

## Development Setup

### Prerequisites

- Rust toolchain (stable) installed via [rustup](https://rustup.rs/)
- Cargo (bundled with rustup)

### Toolchain and MSRV

- Edition: 2021
- MSRV: 1.71.1 (see `rust-version` in `Cargo.toml`)

Please avoid raising MSRV in PRs unless discussed first. If MSRV must increase, mention it explicitly in the PR description and include an entry under the "Unreleased" section of `CHANGELOG.md`.

### Building

```bash
cargo build
```

### Testing

```bash
cargo test --lib --examples --benches
```

CI also builds all targets and runs tests for the library, examples, and benches across Linux/macOS/Windows. If you touch examples or benches, please run the relevant subset locally.

### Linting & Formatting

- Formatting: `cargo fmt --all` (CI uses `cargo fmt --check`)
- Linting: `cargo clippy -- -D warnings`

Try to keep Clippy clean without adding broad `#[allow]` attributes unless there’s a strong reason.

### Documentation

Build docs locally:

```bash
cargo doc --no-deps
```

If you add or change public APIs, please:

- Include rustdoc comments with examples where helpful
- Ensure examples compile and run quickly
- Keep intra-doc links accurate (e.g., [`TypeName`], [`module::Item`])

### Examples

Run examples with Cargo:

```bash
cargo run --example tcp-stream-console-logger
cargo run --example tokio-tcp-stream-console-logger
cargo run --example file-logger
```

Some examples use `env_logger`. You can control verbosity via `RUST_LOG`, for example:

```bash
RUST_LOG=debug cargo run --example tcp-stream-console-logger
```

### Benchmarks

This project uses [Criterion](https://github.com/bheisler/criterion.rs) for benchmarks.

- Run all benches: `cargo bench`
- Run a specific bench: `cargo bench --bench buffer-formatter`

Tips for more stable numbers:

- Close other apps and run on AC power
- Use a consistent CPU governor and temperature; avoid thermal throttling
- Prefer running the same bench multiple times and compare medians
- Criterion offers knobs like `--sample-size` and `--measurement-time` if needed

If you add a benchmark, keep it small, deterministic, and clearly named.

## Project Structure

- `src/` — library source code
- `examples/` — runnable examples
- `benches/` — Criterion benchmarks
- `README.md` — crate overview and usage
- `CHANGELOG.md` — release notes
- `RELEASE.md` — release checklist
- `SECURITY.md` — how to report security issues

## Performance & Reliability

This crate is often used around I/O hot paths, so performance changes matter.

- Prefer small, measurable optimizations over speculative ones.
- Avoid adding allocations on per-read/per-write paths unless justified.
- Benchmarks should be deterministic where practical (avoid measuring setup/teardown).
- If you add `#[inline]` / `#[inline(always)]`, consider both real-world impact and code-size tradeoffs.

When sharing benchmark results in a PR, try to include CPU/OS details, the command used, and a baseline commit.

## Commit & PR Etiquette

- Keep commits focused; prefer small, logically separate changes.
- Use clear, imperative summaries (optionally Conventional Commits, e.g., `feat:`, `fix:`, `docs:`).
- Explain the motivation and approach in the PR description. Link related issues.
- Include tests and/or examples for new features or bug fixes.
- Ensure formatting, Clippy, and tests pass before requesting review.

## Release Process

To prepare and publish a new release, see:

- [RELEASE.md](./RELEASE.md)

## Security

Please do NOT open a public issue for security reports. Follow the instructions in [SECURITY.md](./SECURITY.md).

## License

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you will be dual licensed, without additional terms, under the same terms as the project:

- Apache License, Version 2.0 (see [LICENSE-APACHE](./LICENSE-APACHE))
- MIT License (see [LICENSE-MIT](./LICENSE-MIT))

This mirrors the licensing of the crate itself (MIT OR Apache-2.0).
