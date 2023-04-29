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
