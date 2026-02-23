use crate::Record;
use crate::RecordKind;
use itertools::Itertools;
use std::fmt;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Trait
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Trait for filtering log records in [`LoggedStream`].
///
/// This trait allows filtering log records ([`Record`]) using the [`check`] method, which returns a [`bool`] value.
/// It should be implemented for structures intended to be used as the filtering component within [`LoggedStream`].
///
/// [`check`]: RecordFilter::check
/// [`LoggedStream`]: crate::LoggedStream
pub trait RecordFilter: Send + 'static {
    /// This method returns [`bool`] value depending on if received log record ([`Record`]) should be processed
    /// by logging part inside [`LoggedStream`].
    ///
    /// [`LoggedStream`]: crate::LoggedStream
    fn check(&self, record: &Record) -> bool;

    /// This method provides [`fmt::Debug`] representation of the filter. It is used by composite filters
    /// ([`AllFilter`] and [`AnyFilter`]) to produce detailed debug output of their underlying filters.
    /// The default implementation outputs the type name as `"UnknownFilter"`.
    /// Implementors are encouraged to override this method to provide meaningful debug information.
    fn fmt_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UnknownFilter")
    }
}

impl<T: RecordFilter + ?Sized> RecordFilter for Box<T> {
    fn check(&self, record: &Record) -> bool {
        (**self).check(record)
    }

    fn fmt_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt_debug(f)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// DefaultFilter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// This is default implementation of [`RecordFilter`] trait which [`check`] method always return `true`.
/// It should be constructed using [`Default::default`] method.
///
/// [`check`]: RecordFilter::check
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultFilter;

impl RecordFilter for DefaultFilter {
    #[inline]
    fn check(&self, _record: &Record) -> bool {
        true
    }

    fn fmt_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// RecordKindFilter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Implementation of [`RecordFilter`] that accepts allowed [`RecordKind`] array.
///
/// This implementation of the [`RecordFilter`] trait accepts an array of allowed log record kinds ([`RecordKind`]) during
/// construction. Its [`check`] method returns `true` if the received log record kind is present in this array.
///
/// [`check`]: RecordFilter::check
#[derive(Debug)]
pub struct RecordKindFilter {
    allowed_kinds: Vec<RecordKind>,
}

impl RecordKindFilter {
    /// Construct a new instance of [`RecordKindFilter`] using provided array of allowed log record kinds ([`RecordKind`]).
    pub fn new(kinds: &'static [RecordKind]) -> Self {
        Self {
            allowed_kinds: kinds.iter().copied().unique().collect(),
        }
    }
}

impl RecordFilter for RecordKindFilter {
    #[inline]
    fn check(&self, record: &Record) -> bool {
        self.allowed_kinds.contains(&record.kind)
    }

    fn fmt_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// AllFilter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Implementation of [`RecordFilter`] that combines multiple filters with AND logic.
///
/// This implementation of the [`RecordFilter`] trait accepts a vector of boxed filters during construction.
/// Its [`check`] method returns `true` only if **all** underlying filters return `true` for the given record.
/// If the filter list is empty, it returns `true` by default (empty conjunction is vacuously true).
///
/// This filter is useful for combining multiple filtering conditions where all must be satisfied.
///
/// # Examples
///
/// ```
/// use logged_stream::{AllFilter, RecordKindFilter, RecordFilter, Record, RecordKind};
///
/// // Create a filter that accepts only Read operations
/// let filter = AllFilter::new(vec![
///     Box::new(RecordKindFilter::new(&[RecordKind::Read])),
/// ]);
///
/// let read_record = Record::new(RecordKind::Read, String::from("data"));
/// assert!(filter.check(&read_record));
///
/// let error_record = Record::new(RecordKind::Error, String::from("error"));
/// assert!(!filter.check(&error_record));
/// ```
///
/// [`check`]: RecordFilter::check
pub struct AllFilter {
    filters: Vec<Box<dyn RecordFilter>>,
}

/// Helper wrapper to bridge [`RecordFilter::fmt_debug`] into [`fmt::Debug`].
struct RecordFilterDebug<'a>(&'a dyn RecordFilter);

impl fmt::Debug for RecordFilterDebug<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt_debug(f)
    }
}

/// Helper wrapper to format a slice of [`RecordFilter`]s without allocating.
struct RecordFiltersDebug<'a>(&'a [Box<dyn RecordFilter>]);

impl fmt::Debug for RecordFiltersDebug<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut list = f.debug_list();
        for filter in self.0.iter() {
            list.entry(&RecordFilterDebug(filter.as_ref()));
        }
        list.finish()
    }
}

impl fmt::Debug for AllFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AllFilter")
            .field("filters", &RecordFiltersDebug(&self.filters))
            .finish()
    }
}

impl AllFilter {
    /// Construct a new instance of [`AllFilter`] using provided vector of boxed filters.
    ///
    /// # Arguments
    ///
    /// * `filters` - A vector of boxed filters implementing [`RecordFilter`] trait
    ///
    /// # Examples
    ///
    /// ```
    /// use logged_stream::{AllFilter, RecordKindFilter, RecordKind};
    ///
    /// let filter = AllFilter::new(vec![
    ///     Box::new(RecordKindFilter::new(&[RecordKind::Read])),
    ///     Box::new(RecordKindFilter::new(&[RecordKind::Read, RecordKind::Write])),
    /// ]);
    /// ```
    pub fn new(filters: Vec<Box<dyn RecordFilter>>) -> Self {
        Self { filters }
    }
}

impl RecordFilter for AllFilter {
    #[inline]
    fn check(&self, record: &Record) -> bool {
        self.filters.iter().all(|filter| filter.check(record))
    }

    fn fmt_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// AnyFilter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Implementation of [`RecordFilter`] that combines multiple filters with OR logic.
///
/// This implementation of the [`RecordFilter`] trait accepts a vector of boxed filters during construction.
/// Its [`check`] method returns `true` if **any** of the underlying filters returns `true` for the given record.
/// If the filter list is empty, it returns `false` by default (empty disjunction is false).
///
/// This filter is useful for combining multiple filtering conditions where at least one must be satisfied.
///
/// # Examples
///
/// ```
/// use logged_stream::{AnyFilter, RecordKindFilter, RecordFilter, Record, RecordKind};
///
/// // Create a filter that accepts Read OR Write operations
/// let filter = AnyFilter::new(vec![
///     Box::new(RecordKindFilter::new(&[RecordKind::Read])),
///     Box::new(RecordKindFilter::new(&[RecordKind::Write])),
/// ]);
///
/// let read_record = Record::new(RecordKind::Read, String::from("data"));
/// assert!(filter.check(&read_record));
///
/// let write_record = Record::new(RecordKind::Write, String::from("data"));
/// assert!(filter.check(&write_record));
///
/// let error_record = Record::new(RecordKind::Error, String::from("error"));
/// assert!(!filter.check(&error_record));
/// ```
///
/// [`check`]: RecordFilter::check
pub struct AnyFilter {
    filters: Vec<Box<dyn RecordFilter>>,
}

impl fmt::Debug for AnyFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnyFilter")
            .field(
                "filters",
                &self
                    .filters
                    .iter()
                    .map(|filter| RecordFilterDebug(filter.as_ref()))
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl AnyFilter {
    /// Construct a new instance of [`AnyFilter`] using provided vector of boxed filters.
    ///
    /// # Arguments
    ///
    /// * `filters` - A vector of boxed filters implementing [`RecordFilter`] trait
    ///
    /// # Examples
    ///
    /// ```
    /// use logged_stream::{AnyFilter, RecordKindFilter, RecordKind};
    ///
    /// let filter = AnyFilter::new(vec![
    ///     Box::new(RecordKindFilter::new(&[RecordKind::Read])),
    ///     Box::new(RecordKindFilter::new(&[RecordKind::Write])),
    /// ]);
    /// ```
    pub fn new(filters: Vec<Box<dyn RecordFilter>>) -> Self {
        Self { filters }
    }
}

impl RecordFilter for AnyFilter {
    #[inline]
    fn check(&self, record: &Record) -> bool {
        self.filters.iter().any(|filter| filter.check(record))
    }

    fn fmt_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Tests
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use crate::filter::AllFilter;
    use crate::filter::AnyFilter;
    use crate::filter::DefaultFilter;
    use crate::filter::RecordFilter;
    use crate::filter::RecordKindFilter;
    use crate::record::Record;
    use crate::record::RecordKind;
    use std::fmt;

    fn assert_unpin<T: Unpin>() {}

    #[test]
    fn test_unpin() {
        assert_unpin::<DefaultFilter>();
        assert_unpin::<RecordKindFilter>();
        assert_unpin::<AllFilter>();
        assert_unpin::<AnyFilter>();
    }

    #[test]
    fn test_default_filter() {
        assert!(DefaultFilter.check(&Record::new(
            RecordKind::Read,
            String::from("01:02:03:04:05:06")
        )));
        assert!(DefaultFilter.check(&Record::new(
            RecordKind::Write,
            String::from("01:02:03:04:05:06")
        )));
        assert!(DefaultFilter.check(&Record::new(RecordKind::Drop, String::from("deallocated"))));
        assert!(DefaultFilter.check(&Record::new(
            RecordKind::Shutdown,
            String::from("write shutdown request")
        )));
    }

    #[test]
    fn test_record_kind_filter() {
        let filter = RecordKindFilter::new(&[RecordKind::Read]);
        assert!(filter.check(&Record::new(
            RecordKind::Read,
            String::from("01:02:03:04:05:06")
        )));
        assert!(!filter.check(&Record::new(
            RecordKind::Write,
            String::from("01:02:03:04:05:06")
        )));
        assert!(!filter.check(&Record::new(RecordKind::Drop, String::from("deallocated"))));
        assert!(!filter.check(&Record::new(
            RecordKind::Shutdown,
            String::from("write shutdown request")
        )));
    }

    #[test]
    fn test_all_filter_empty() {
        let filter = AllFilter::new(vec![]);
        // Empty conjunction should return true (vacuously true)
        assert!(filter.check(&Record::new(RecordKind::Read, String::from("test"))));
        assert!(filter.check(&Record::new(RecordKind::Write, String::from("test"))));
        assert!(filter.check(&Record::new(RecordKind::Error, String::from("test"))));
    }

    #[test]
    fn test_all_filter_single() {
        let filter = AllFilter::new(vec![Box::new(RecordKindFilter::new(&[RecordKind::Read]))]);

        assert!(filter.check(&Record::new(RecordKind::Read, String::from("data"))));
        assert!(!filter.check(&Record::new(RecordKind::Write, String::from("data"))));
        assert!(!filter.check(&Record::new(RecordKind::Error, String::from("error"))));
    }

    #[test]
    fn test_all_filter_multiple_all_pass() {
        // Both filters accept Read
        let filter = AllFilter::new(vec![
            Box::new(RecordKindFilter::new(&[
                RecordKind::Read,
                RecordKind::Write,
            ])),
            Box::new(RecordKindFilter::new(&[
                RecordKind::Read,
                RecordKind::Error,
            ])),
        ]);

        assert!(filter.check(&Record::new(RecordKind::Read, String::from("data"))));
    }

    #[test]
    fn test_all_filter_multiple_one_fails() {
        // First accepts Write, second doesn't
        let filter = AllFilter::new(vec![
            Box::new(RecordKindFilter::new(&[
                RecordKind::Read,
                RecordKind::Write,
            ])),
            Box::new(RecordKindFilter::new(&[
                RecordKind::Read,
                RecordKind::Error,
            ])),
        ]);

        assert!(!filter.check(&Record::new(RecordKind::Write, String::from("data"))));
    }

    #[test]
    fn test_all_filter_multiple_all_fail() {
        let filter = AllFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(RecordKindFilter::new(&[RecordKind::Write])),
        ]);

        assert!(!filter.check(&Record::new(RecordKind::Error, String::from("error"))));
    }

    #[test]
    fn test_all_filter_with_default() {
        // Combining with DefaultFilter (which always returns true)
        let filter = AllFilter::new(vec![
            Box::new(DefaultFilter),
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
        ]);

        assert!(filter.check(&Record::new(RecordKind::Read, String::from("data"))));
        assert!(!filter.check(&Record::new(RecordKind::Write, String::from("data"))));
    }

    #[test]
    fn test_any_filter_empty() {
        let filter = AnyFilter::new(vec![]);
        // Empty disjunction should return false
        assert!(!filter.check(&Record::new(RecordKind::Read, String::from("test"))));
        assert!(!filter.check(&Record::new(RecordKind::Write, String::from("test"))));
        assert!(!filter.check(&Record::new(RecordKind::Error, String::from("test"))));
    }

    #[test]
    fn test_any_filter_single() {
        let filter = AnyFilter::new(vec![Box::new(RecordKindFilter::new(&[RecordKind::Read]))]);

        assert!(filter.check(&Record::new(RecordKind::Read, String::from("data"))));
        assert!(!filter.check(&Record::new(RecordKind::Write, String::from("data"))));
        assert!(!filter.check(&Record::new(RecordKind::Error, String::from("error"))));
    }

    #[test]
    fn test_any_filter_multiple_first_passes() {
        let filter = AnyFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(RecordKindFilter::new(&[RecordKind::Write])),
        ]);

        assert!(filter.check(&Record::new(RecordKind::Read, String::from("data"))));
    }

    #[test]
    fn test_any_filter_multiple_second_passes() {
        let filter = AnyFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(RecordKindFilter::new(&[RecordKind::Write])),
        ]);

        assert!(filter.check(&Record::new(RecordKind::Write, String::from("data"))));
    }

    #[test]
    fn test_any_filter_multiple_all_pass() {
        let filter = AnyFilter::new(vec![
            Box::new(RecordKindFilter::new(&[
                RecordKind::Read,
                RecordKind::Write,
            ])),
            Box::new(RecordKindFilter::new(&[
                RecordKind::Read,
                RecordKind::Error,
            ])),
        ]);

        assert!(filter.check(&Record::new(RecordKind::Read, String::from("data"))));
    }

    #[test]
    fn test_any_filter_multiple_all_fail() {
        let filter = AnyFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(RecordKindFilter::new(&[RecordKind::Write])),
        ]);

        assert!(!filter.check(&Record::new(RecordKind::Error, String::from("error"))));
    }

    #[test]
    fn test_any_filter_with_default() {
        // Combining with DefaultFilter (which always returns true)
        let filter = AnyFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(DefaultFilter),
        ]);

        // Should pass for everything because DefaultFilter always returns true
        assert!(filter.check(&Record::new(RecordKind::Read, String::from("data"))));
        assert!(filter.check(&Record::new(RecordKind::Write, String::from("data"))));
        assert!(filter.check(&Record::new(RecordKind::Error, String::from("error"))));
    }

    #[test]
    fn test_nested_composite_filters() {
        // (Read OR Write)
        // Implemented as: AllFilter containing AnyFilter for (Read OR Write)
        let filter = AllFilter::new(vec![Box::new(AnyFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(RecordKindFilter::new(&[RecordKind::Write])),
        ]))]);

        assert!(filter.check(&Record::new(RecordKind::Read, String::from("data"))));
        assert!(filter.check(&Record::new(RecordKind::Write, String::from("data"))));
        assert!(!filter.check(&Record::new(RecordKind::Drop, String::from("dropped"))));
        assert!(!filter.check(&Record::new(RecordKind::Error, String::from("error"))));
    }

    #[test]
    fn test_complex_nested_filters() {
        // AllFilter containing two AnyFilters:
        // (Read OR Write) AND (Read OR Error)
        // This should only pass for Read
        let filter = AllFilter::new(vec![
            Box::new(AnyFilter::new(vec![
                Box::new(RecordKindFilter::new(&[RecordKind::Read])),
                Box::new(RecordKindFilter::new(&[RecordKind::Write])),
            ])),
            Box::new(AnyFilter::new(vec![
                Box::new(RecordKindFilter::new(&[RecordKind::Read])),
                Box::new(RecordKindFilter::new(&[RecordKind::Error])),
            ])),
        ]);

        assert!(filter.check(&Record::new(RecordKind::Read, String::from("data"))));
        assert!(!filter.check(&Record::new(RecordKind::Write, String::from("data"))));
        assert!(!filter.check(&Record::new(RecordKind::Error, String::from("error"))));
        assert!(!filter.check(&Record::new(RecordKind::Drop, String::from("dropped"))));
    }

    #[test]
    fn test_trait_object_safety() {
        // Assert trait object construct.
        let default: Box<dyn RecordFilter> = Box::<DefaultFilter>::default();
        let record_kind: Box<dyn RecordFilter> = Box::new(RecordKindFilter::new(&[]));
        let all: Box<dyn RecordFilter> = Box::new(AllFilter::new(vec![]));
        let any: Box<dyn RecordFilter> = Box::new(AnyFilter::new(vec![]));

        let record = Record::new(RecordKind::Open, String::from("test log record"));

        // Assert that trait object methods are dispatchable.
        _ = default.check(&record);
        _ = record_kind.check(&record);
        _ = all.check(&record);
        _ = any.check(&record);
    }

    fn assert_record_filter<T: RecordFilter>() {}

    #[test]
    fn test_box() {
        assert_record_filter::<Box<dyn RecordFilter>>();
        assert_record_filter::<Box<RecordKindFilter>>();
        assert_record_filter::<Box<DefaultFilter>>();
        assert_record_filter::<Box<AllFilter>>();
        assert_record_filter::<Box<AnyFilter>>();
    }

    fn assert_send<T: Send>() {}

    #[test]
    fn test_send() {
        assert_send::<RecordKindFilter>();
        assert_send::<DefaultFilter>();
        assert_send::<AllFilter>();
        assert_send::<AnyFilter>();

        assert_send::<Box<dyn RecordFilter>>();
        assert_send::<Box<RecordKindFilter>>();
        assert_send::<Box<DefaultFilter>>();
        assert_send::<Box<AllFilter>>();
        assert_send::<Box<AnyFilter>>();
    }

    fn assert_debug<T: fmt::Debug>() {}

    #[test]
    fn test_debug() {
        assert_debug::<DefaultFilter>();
        assert_debug::<RecordKindFilter>();
        assert_debug::<AllFilter>();
        assert_debug::<AnyFilter>();

        assert_debug::<Box<DefaultFilter>>();
        assert_debug::<Box<RecordKindFilter>>();
        assert_debug::<Box<AllFilter>>();
        assert_debug::<Box<AnyFilter>>();
    }

    #[test]
    fn test_debug_output() {
        // DefaultFilter
        assert_eq!(format!("{:?}", DefaultFilter), "DefaultFilter");

        // RecordKindFilter
        let record_kind_filter = RecordKindFilter::new(&[RecordKind::Read, RecordKind::Write]);
        let debug_str = format!("{:?}", record_kind_filter);
        assert!(debug_str.contains("RecordKindFilter"));
        assert!(debug_str.contains("Read"));
        assert!(debug_str.contains("Write"));

        // AllFilter (empty)
        let all_filter_empty = AllFilter::new(vec![]);
        let debug_str = format!("{:?}", all_filter_empty);
        assert!(debug_str.contains("AllFilter"));
        assert!(debug_str.contains("filters: []"));

        // AllFilter (with children)
        let all_filter = AllFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(DefaultFilter),
        ]);
        let debug_str = format!("{:?}", all_filter);
        assert!(debug_str.contains("AllFilter"));
        assert!(debug_str.contains("RecordKindFilter"));
        assert!(debug_str.contains("Read"));
        assert!(debug_str.contains("DefaultFilter"));

        // AnyFilter (empty)
        let any_filter_empty = AnyFilter::new(vec![]);
        let debug_str = format!("{:?}", any_filter_empty);
        assert!(debug_str.contains("AnyFilter"));
        assert!(debug_str.contains("filters: []"));

        // AnyFilter (with children)
        let any_filter = AnyFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Write])),
            Box::new(DefaultFilter),
        ]);
        let debug_str = format!("{:?}", any_filter);
        assert!(debug_str.contains("AnyFilter"));
        assert!(debug_str.contains("RecordKindFilter"));
        assert!(debug_str.contains("Write"));
        assert!(debug_str.contains("DefaultFilter"));

        // Nested composite filter
        let nested = AllFilter::new(vec![Box::new(AnyFilter::new(vec![Box::new(
            RecordKindFilter::new(&[RecordKind::Read]),
        )]))]);
        let debug_str = format!("{:?}", nested);
        assert!(debug_str.contains("AllFilter"));
        assert!(debug_str.contains("AnyFilter"));
        assert!(debug_str.contains("RecordKindFilter"));
        assert!(debug_str.contains("Read"));
    }
}
