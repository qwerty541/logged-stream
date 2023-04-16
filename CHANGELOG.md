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
