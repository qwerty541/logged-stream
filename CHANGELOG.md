## v0.2.0 (14.04.2023)

-   Extend `LoggedStream` structure with fourth part which will be responsible for log records filter.
-   Added new trait `RecordFilter` which must be implemented by new fourth part of `LoggedStream`.
-   Added several implementations of `RecordFilter` trait: `DefaultFilter` which accepts all log records and `RecordKindFilter` which accepts log records with kinds specified during construct.
-   Improved several sections inside README file.
-   Removed redundant dependencies features.