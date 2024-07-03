# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

## v0.3.5 (24.02.2024)

- Dependencies updates:
  - `itertools` from 0.11.0 to 0.12.1
  - `tokio` from 1.32.0 to 1.36.0
  - `env_logger` from 0.10.0 to 0.10.1
  - `chrono` from 0.4.31 to 0.4.34

## v0.3.4 (07.10.2023)

- Bump minimal supported rust version (MSRV) from 1.60.0 to 1.63.0
- README improvements.
- Dependencies updates:
  - `tokio` from 1.31.0 to 1.32.0
  - `chrono` from 0.4.26 to 0.4.31

## v0.3.3 (13.08.2023)

-   Bump `itertools` from 0.10.5 to 0.11.0
-   Bump `log` from 0.4.18 to 0.4.20
-   Bump `tokio` from 1.28.2 to 1.31.0

## v0.3.2 (12.06.2023)

-   Changed `new` method signature of all `BufferFormatter` trait implementations.

Before:

```rust
pub fn new(provided_separator: Option<&'static str>) -> Self;
```

After:

```rust
pub fn new(provided_separator: Option<&str>) -> Self;
```

- Updated some dependencies.

## v0.3.1 (09.06.2023)

-   Changed `BufferFormatter::get_separator` method signature.

Before:

```rust
fn get_separator(&self) -> &'static str;
```

After:

```rust
fn get_separator(&self) -> &str;
```

-   Added `new_owned` and `new_default` methods for every `BufferFormatter` trait implementation.
-   Implemented `Default` trait for every `BufferFormatter` trait implementation.
-   Cover more code with tests.
-   Improved examples section inside README file.

## v0.3.0 (30.05.2023)

-   Add documentation for all public (exported) items (issue #10).
-   Split `HexadecimalFormatter` into lowercase and uppercase (issue #12).
-   Cover more code with tests.

## v0.2.5 (02.05.2023)

-   Implemented `BufferFormatter`, `RecordFilter` and `Logger` traits for boxed structures, which already implement such traits.
-   Implemented such traits for their boxed trait objects.
-   Such trait now required to be `Send`.
-   Covered with test all changes above.

## v0.2.4 (29.04.2023)

-   Traits `BufferFormatter`, `RecordFilter` and `Logger` now are object safe and do not require `Sized` implementation. This is the same change as in the previous minor version, but done correctly and covered with tests. Unfortunately I had a misunderstanding of trait object safety.

## v0.2.3 (26.04.2023)

-   Structures which implement `BufferFormatter`, `RecordFilter` and `Logger` traits now required to be `Sized`. This change allows to use the following traits as trait-objects i.e. `Box<dyn BufferFormatter>`.

## v0.2.2 (18.04.2023)

-   Add categories into `Cargo.toml` file.
-   Exclude examples folder from published package to decrease its size.
-   README improvements.

## v0.2.1 (17.04.2023)

-   Exclude several redundant files and folders from published package to decrease its size.
-   `ConsoleLogger` now does not include timestamp into log string, it can be done by `env_logger`.
-   `ConsoleLogger` now ignores provided level when receive error kind log records.
-   Several README improvements.

## v0.2.0 (14.04.2023)

-   Extend `LoggedStream` structure with fourth part which will be responsible for log records filter.
-   Added new trait `RecordFilter` which must be implemented by new fourth part of `LoggedStream`.
-   Added several implementations of `RecordFilter` trait: `DefaultFilter` which accepts all log records and `RecordKindFilter` which accepts log records with kinds specified during construct.
-   Improved several sections inside README file.
-   Removed redundant dependencies features.

## v0.1.0 (13.04.2023)

Initial release
