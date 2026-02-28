# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.5.0 (28.02.2026)

### Added

- Added `AllFilter` and `AnyFilter` composite filters for combining multiple `RecordFilter`s with AND and OR logic respectively.
- Added `new_static` method to all `BufferFormatter` structures to allow creating instances with static string separators.
- Implemented `fmt::Display`, `Eq`, `PartialEq` and `Hash` for all `BufferFormatter` structures.
- Implemented `From<Cow<'static, str>>`, `From<&str>`, `From<String>`, `From<Option<&str>>` and `From<Option<String>>` for all `BufferFormatter` structures.
- Implemented `BufferFormatter` for `Arc<T>` where `T: BufferFormatter + ?Sized + Sync`.

### Changed

- All `BufferFormatter` structures now use `Cow<'static, str>` under the hood for memory optimization and flexibility.
- Refactored `BufferFormatter` structures using a macro-based generation approach to reduce code duplication and improve maintainability.

### Documentation

- Created security protocol document with instructions for reporting security vulnerabilities.
- Created basic contributing document with instructions for contributing to the project.
- Updated license badge to be clickable and link to the license file in the repository.
- Updated Rust version badge to be clickable.
- Added total lines count badge to the README file.
- Added COCOMO badge to the README file.

### Dependencies

- Updated `bytes` from 1.4.0 to 1.11.1
- Updated `tokio` from 1.45.0 to 1.49.0
- Updated `slab` from 0.4.10 to 0.4.11
- Updated `chrono` from 0.4.41 to 0.4.43
- Updated `log` from 0.4.27 to 0.4.29
- Updated `criterion` from 0.5.1 to 0.8.2

## v0.4.1 (15.05.2025)

### Changed

- Changed MSRV from 1.63.0 to 1.71.1.

### Fixed

- Resolved multiple `too_long_first_doc_paragraph` clippy warnings on the nightly toolchain.

### Documentation

- Created code of conduct document.
- Added badge with total crate downloads count to the README file.
- Minor README improvements.

### Dependencies

- Updated `log` from 0.4.21 to 0.4.27
- Updated `tokio` from 1.38.0 to 1.45.0
- Updated `env_logger` from 0.10.2 to 0.11.6
- Updated `chrono` from 0.4.38 to 0.4.41
- Updated `itertools` from 0.13.0 to 0.14.0

---

## v0.4.0 (03.07.2024)

### Added

- Added `FileLogger` structure which is the new `Logger` trait implementation, it writes log records into a file.
- Created an example of `FileLogger` structure usage.
- Created basic performance tests for `BufferFormatter` and `RecordFilter` traits implementations using `criterion` library.
- Added `#[inline]` attribute to several methods to improve performance based on the results of performance tests.

### Fixed

- Removed unnecessary imports to fix clippy lints.
- Fixed indentation of documentation comments in several places to fix `doc_lazy_continuation` clippy lint.

### Documentation

- Table of contents currently hidden by default.
- Extended description section with use cases information.

### Dependencies

- Updated `itertools` from 0.12.1 to 0.13.0
- Updated `mio` from 0.8.10 to 0.8.11
- Updated `log` from 0.4.20 to 0.4.21
- Updated `chrono` from 0.4.34 to 0.4.38
- Updated `tokio` from 1.36.0 to 1.38.0

---

## v0.3.5 (24.02.2024)

### Dependencies

- Updated `itertools` from 0.11.0 to 0.12.1.
- Updated `tokio` from 1.32.0 to 1.36.0.
- Updated `env_logger` from 0.10.0 to 0.10.1.
- Updated `chrono` from 0.4.31 to 0.4.34.

---

## v0.3.4 (07.10.2023)

### Changed

- Changed MSRV from 1.60.0 to 1.63.0.

### Documentation

- Improved README file.

### Dependencies

- Updated `tokio` from 1.31.0 to 1.32.0.
- Updated `chrono` from 0.4.26 to 0.4.31.

---

## v0.3.3 (13.08.2023)

### Dependencies

- Updated `itertools` from 0.10.5 to 0.11.0.
- Updated `log` from 0.4.18 to 0.4.20.
- Updated `tokio` from 1.28.2 to 1.31.0.

---

## v0.3.2 (12.06.2023)

### Changed

- Updated the `new` method signature for all `BufferFormatter` trait implementations.

  **Before:**
  ```rust
  pub fn new(provided_separator: Option<&'static str>) -> Self;
  ```

  **After:**
  ```rust
  pub fn new(provided_separator: Option<&str>) -> Self;
  ```

### Dependencies

- Updated several dependencies.

---

## v0.3.1 (09.06.2023)

### Added

- Added `new_owned` and `new_default` methods for all `BufferFormatter` trait implementations.
- Implemented the `Default` trait for all `BufferFormatter` trait implementations.

### Changed

- Updated the `BufferFormatter::get_separator` method signature.

  **Before:**
  ```rust
  fn get_separator(&self) -> &'static str;
  ```

  **After:**
  ```rust
  fn get_separator(&self) -> &str;
  ```

### Testing

- Increased test coverage.

### Documentation

- Improved the examples section in the README file.

---

## v0.3.0 (30.05.2023)

### Added

- Added documentation for all public (exported) items (issue #10).
- Split `HexadecimalFormatter` into lowercase and uppercase variants (issue #12).

### Testing

- Increased test coverage.

---

## v0.2.5 (02.05.2023)

### Added

- Implemented `BufferFormatter`, `RecordFilter`, and `Logger` traits for boxed structures that already implement these traits.
- Implemented these traits for their boxed trait objects.
- Made these traits require the `Send` marker.

### Testing

- Added tests to cover the above changes.

---

## v0.2.4 (29.04.2023)

### Changed

- Made the `BufferFormatter`, `RecordFilter`, and `Logger` traits object-safe and removed the `Sized` requirement.
- Corrected the implementation of the above change and added tests (fix for issues introduced in `v0.2.3`).

---

## v0.2.3 (26.04.2023)

### Changed

- Made structures implementing the `BufferFormatter`, `RecordFilter`, and `Logger` traits require the `Sized` marker.
- Enabled the use of these traits as trait objects (e.g., `Box<dyn BufferFormatter>`).

---

## v0.2.2 (18.04.2023)

### Added

- Added categories to the `Cargo.toml` file.

### Changed

- Excluded the `examples` folder from the published package to reduce its size.

### Documentation

- Improved the README file.

---

## v0.2.1 (17.04.2023)

### Changed

- Excluded redundant files and folders from the published package to reduce its size.
- Updated `ConsoleLogger` to exclude timestamps from log strings (can now be handled by `env_logger`).
- Updated `ConsoleLogger` to ignore the provided log level for error-kind log records.

### Documentation

- Improved the README file.

---

## v0.2.0 (14.04.2023)

### Added

- Extended the `LoggedStream` structure with a fourth component for log record filtering.
- Introduced the `RecordFilter` trait for the new filtering component.
- Added implementations of the `RecordFilter` trait:
  - `DefaultFilter`: Accepts all log records.
  - `RecordKindFilter`: Accepts log records of specified kinds.

### Documentation

- Improved several sections in the README file.

### Removed

- Removed redundant dependency features.

---

## v0.1.0 (13.04.2023)

### Initial Release

- Initial release of the `logged-stream` crate.
