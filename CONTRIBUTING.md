# Contributing to logged-stream <!-- omit in toc -->

<details>
<summary>Table of contents</summary>

- [Description](#description)
- [Quick start checklist](#quick-start-checklist)
- [Project layout](#project-layout)
- [Toolchain and MSRV](#toolchain-and-msrv)
- [Building, testing, and docs](#building-testing-and-docs)
- [Linting and formatting](#linting-and-formatting)
- [Examples](#examples)
- [Benchmarks](#benchmarks)
- [Commit messages and PRs](#commit-messages-and-prs)
- [Filing issues](#filing-issues)
- [Security](#security)
- [License](#license)
</details>

## Description

Thanks for your interest in improving logged-stream! This document explains how to get set up, propose changes, and meet the project’s quality bar. Contributions of all kinds are welcome: bug reports, docs, code, and examples.

This project follows a Code of Conduct; by participating you agree to abide by it. See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Quick start checklist

- Fork and clone the repo.
- Install Rust via rustup (stable toolchain).
- Ensure you meet the MSRV (minimum supported Rust version): Rust 1.71.1 or newer.
- Build and run tests locally:
  - Format: `cargo fmt --all`
  - Lint: `cargo clippy --all-targets --all-features -- -D warnings`
  - Test: `cargo test --all-features`
  - Docs (optional): `cargo doc --no-deps`
- If you change public behavior/APIs, add/update docs, examples, and CHANGELOG entries.

## Project layout

- `src/` – library code
- `examples/` – runnable examples
- `benches/` – Criterion benchmarks
- `CHANGELOG.md` – release notes (Keep a Changelog)
- `SECURITY.md` – how to report security issues

## Toolchain and MSRV

- Edition: 2021
- MSRV: 1.71.1 (see `rust-version` in Cargo.toml)
- Please avoid raising MSRV in PRs unless discussed. If MSRV must increase, call it out in the PR description and add a `Changed` note under the "Unreleased" section of the changelog.

## Building, testing, and docs

- Build: `cargo build`
- Tests: `cargo test --all-features`
- Doc tests: included in `cargo test`
- Documentation: `cargo doc --no-deps`

If you add or change public APIs, please:
- Include rustdoc comments with examples where helpful.
- Ensure examples compile and run quickly.
- Keep intra-doc links accurate (e.g., [`TypeName`], [`module::Item`]).

## Linting and formatting

- This repo uses `rustfmt` (see `rustfmt.toml`). Run: `cargo fmt --all`.
- Run Clippy and treat warnings as errors locally: `cargo clippy --all-targets --all-features -- -D warnings`.
- Keep code idiomatic and small, focused changes where possible.

## Examples

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

## Benchmarks

This project uses [Criterion](https://github.com/bheisler/criterion.rs) for benchmarks.

- Run all benches: `cargo bench`
- Run a specific bench: `cargo bench --bench buffer-formatter`

Tips for more stable numbers:
- Close other apps and run on AC power.
- Use a consistent CPU governor and temperature; avoid thermal throttling.
- Prefer running the same bench multiple times and compare medians.
- Criterion offers knobs like `--sample-size` and `--measurement-time` if needed.

If you add a benchmark, keep it small, deterministic, and clearly named.

## Commit messages and PRs

- Keep commits focused; prefer small, logically separate changes.
- Use clear, imperative summaries (optionally Conventional Commits, e.g., `feat:`, `fix:`, `docs:`).
- Explain the motivation and approach in the PR description. Link related issues.
- Include tests and/or examples for new features or bug fixes.
- Ensure `fmt`, `clippy`, and tests pass before requesting review.

## Filing issues

- Search existing issues first.
- Provide a minimal reproduction if possible (code snippet, steps, expected vs. actual behavior, versions).
- Include platform info (OS, Rust version, crate version).

## Security

Please do NOT open a public issue for security reports. Follow the instructions in [SECURITY.md](SECURITY.md).

## License

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you shall be dual licensed, without additional terms, under the same terms as the project:

- Apache License, Version 2.0 (see [LICENSE-APACHE](LICENSE-APACHE))
- MIT License (see [LICENSE-MIT](LICENSE-MIT))

This mirrors the licensing of the crate itself (MIT OR Apache-2.0).
