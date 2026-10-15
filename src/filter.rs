use crate::RawRecord;
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
/// # Formatted and raw filtering
///
/// [`check`] judges the formatted [`Record`], the human-readable form you see in the log, and it is the only
/// method a filter has to implement. Reads and writes are offered to [`check_raw`] instead, as a [`RawRecord`]
/// carrying the record kind and the raw bytes. Its default implementation hands the formatted record to
/// [`check`], so a filter that implements only [`check`] sees every record exactly as before. Override
/// [`check_raw`] to decide reads and writes on their kind or their raw bytes; with lazy formatting enabled on the
/// stream ([`LoggedStream::with_lazy_formatting`]), the ones your filter rejects are then not formatted at all.
///
/// # Wrapping other filters
///
/// A filter that wraps or combines other filters must forward [`check_raw`] to them as well as [`check`], as
/// `Box`, [`AllFilter`] and [`AnyFilter`] do. Otherwise the default [`check_raw`] sends reads and writes to the
/// wrapped filters' [`check`], bypassing any decision they take in [`check_raw`].
///
/// [`check`]: RecordFilter::check
/// [`check_raw`]: RecordFilter::check_raw
/// [`LoggedStream`]: crate::LoggedStream
/// [`LoggedStream::with_lazy_formatting`]: crate::LoggedStream::with_lazy_formatting
pub trait RecordFilter: Send + 'static {
    /// This method returns [`bool`] value depending on if received log record ([`Record`]) should be processed
    /// by logging part inside [`LoggedStream`].
    ///
    /// For reads and writes, [`LoggedStream`] calls [`check_raw`], which calls this method unless it is
    /// overridden.
    ///
    /// [`check_raw`]: RecordFilter::check_raw
    /// [`LoggedStream`]: crate::LoggedStream
    fn check(&self, record: &Record) -> bool;

    /// This method returns [`bool`] value depending on if a read or write ([`RawRecord`]) should be processed by
    /// logging part inside [`LoggedStream`], judged on its kind or its raw bytes.
    ///
    /// [`LoggedStream`] calls this method for every read and write; every other record (errors, shutdowns, drops
    /// and `Open` markers) goes to [`check`]. The default implementation formats the record with
    /// [`RawRecord::record`] and returns what [`check`] says, so a filter that does not override this method
    /// behaves exactly as if it did not exist.
    ///
    /// Override it to decide on [`RawRecord::kind`] or the raw [`RawRecord::bytes`], for example to match a
    /// binary protocol header that the formatted message would obscure. With lazy formatting enabled on the
    /// stream ([`LoggedStream::with_lazy_formatting`]), a read or write that your implementation rejects without
    /// calling [`RawRecord::record`] is not formatted at all, and one it accepts is formatted once, for the logger.
    ///
    /// Keep [`check`] consistent with this method where you can: [`LoggedStream`] asks [`check`] only about
    /// records without bytes, but direct callers, and wrappers that do not forward this method, ask it about reads
    /// and writes too.
    ///
    /// # Examples
    ///
    /// ```
    /// use logged_stream::LoggedStream;
    /// use logged_stream::LowercaseHexadecimalFormatter;
    /// use logged_stream::MemoryStorageLogger;
    /// use logged_stream::RawRecord;
    /// use logged_stream::Record;
    /// use logged_stream::RecordFilter;
    /// use logged_stream::RecordKind;
    /// use std::io::Cursor;
    /// use std::io::Read;
    ///
    /// /// Keeps reads and writes that start with a TLS handshake byte (0x16), and every other record.
    /// struct TlsHandshakes;
    ///
    /// impl RecordFilter for TlsHandshakes {
    ///     fn check(&self, record: &Record) -> bool {
    ///         // `LoggedStream` brings only errors, shutdowns, drops and opens here. Direct callers and
    ///         // wrappers may bring reads and writes too: judge those on the hexadecimal message.
    ///         match record.kind {
    ///             RecordKind::Read | RecordKind::Write => record.message.starts_with("16"),
    ///             _ => true,
    ///         }
    ///     }
    ///
    ///     fn check_raw(&self, record: &RawRecord<'_>) -> bool {
    ///         record.bytes().first() == Some(&0x16)
    ///     }
    /// }
    ///
    /// let mut stream = LoggedStream::new(
    ///     Cursor::new(vec![0x16, 0x03, 0x01, 0x17, 0x03]),
    ///     LowercaseHexadecimalFormatter::new_default(),
    ///     TlsHandshakes,
    ///     MemoryStorageLogger::new(16),
    /// )
    /// .with_lazy_formatting(true);
    ///
    /// let mut handshake = [0u8; 3];
    /// stream.read_exact(&mut handshake)?; // kept
    /// let mut application_data = [0u8; 2];
    /// stream.read_exact(&mut application_data)?; // rejected without being formatted
    ///
    /// let records = stream.get_log_records();
    /// assert_eq!(records.len(), 1);
    /// assert_eq!(records[0].message, "16:03:01");
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// [`check`]: RecordFilter::check
    /// [`check_raw`]: RecordFilter::check_raw
    /// [`LoggedStream`]: crate::LoggedStream
    /// [`LoggedStream::with_lazy_formatting`]: crate::LoggedStream::with_lazy_formatting
    fn check_raw(&self, record: &RawRecord<'_>) -> bool {
        self.check(record.record())
    }

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

    // Forwarding is required for correctness, not just speed: without it a boxed filter that decides reads and
    // writes in `check_raw` would be judged by its `check` instead.
    fn check_raw(&self, record: &RawRecord<'_>) -> bool {
        (**self).check_raw(record)
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

    #[inline]
    fn check_raw(&self, _record: &RawRecord<'_>) -> bool {
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
/// Reads and writes are decided from their kind alone in [`check_raw`], without asking for the formatted record.
///
/// [`check`]: RecordFilter::check
/// [`check_raw`]: RecordFilter::check_raw
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

    #[inline]
    fn check_raw(&self, record: &RawRecord<'_>) -> bool {
        self.allowed_kinds.contains(&record.kind())
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
/// Reads and writes go through [`check_raw`] of the underlying filters the same way, one by one in order. They
/// share one [`RawRecord`], so it is formatted at most once however many of them ask for the formatted record.
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
/// [`check_raw`]: RecordFilter::check_raw
pub struct AllFilter {
    filters: Vec<Box<dyn RecordFilter>>,
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

    #[inline]
    fn check_raw(&self, record: &RawRecord<'_>) -> bool {
        self.filters.iter().all(|filter| filter.check_raw(record))
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
/// Reads and writes go through [`check_raw`] of the underlying filters the same way, one by one in order. They
/// share one [`RawRecord`], so it is formatted at most once however many of them ask for the formatted record.
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
/// [`check_raw`]: RecordFilter::check_raw
pub struct AnyFilter {
    filters: Vec<Box<dyn RecordFilter>>,
}

impl fmt::Debug for AnyFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnyFilter")
            .field("filters", &RecordFiltersDebug(&self.filters))
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

    #[inline]
    fn check_raw(&self, record: &RawRecord<'_>) -> bool {
        self.filters.iter().any(|filter| filter.check_raw(record))
    }

    fn fmt_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Debug Helpers
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

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

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Tests
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use crate::buffer_formatter::BufferFormatter;
    use crate::buffer_formatter::LowercaseHexadecimalFormatter;
    use crate::filter::AllFilter;
    use crate::filter::AnyFilter;
    use crate::filter::DefaultFilter;
    use crate::filter::RecordFilter;
    use crate::filter::RecordFilterDebug;
    use crate::filter::RecordKindFilter;
    use crate::record::RawRecord;
    use crate::record::Record;
    use crate::record::RecordKind;
    use std::cell::Cell;
    use std::fmt;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    /// Formats like [`LowercaseHexadecimalFormatter`] and counts its `format_buffer` calls.
    struct CountingFormatter(Cell<usize>);

    impl BufferFormatter for CountingFormatter {
        fn get_separator(&self) -> &str {
            ":"
        }

        fn format_byte(&self, byte: &u8) -> String {
            format!("{byte:02x}")
        }

        fn format_buffer(&self, buffer: &[u8]) -> String {
            self.0.set(self.0.get() + 1);
            LowercaseHexadecimalFormatter::new_default().format_buffer(buffer)
        }
    }

    /// Judges only the formatted message: keeps records whose message contains the needle.
    struct MessageFilter(&'static str);

    impl RecordFilter for MessageFilter {
        fn check(&self, record: &Record) -> bool {
            record.message.contains(self.0)
        }
    }

    /// Decides reads and writes on their first raw byte. Its `check` keeps everything, so a test can tell which
    /// of the two methods made a decision.
    struct FirstByteFilter(u8);

    impl RecordFilter for FirstByteFilter {
        fn check(&self, _record: &Record) -> bool {
            true
        }

        fn check_raw(&self, record: &RawRecord<'_>) -> bool {
            record.bytes().first() == Some(&self.0)
        }
    }

    /// Accepts everything through `check` and counts its calls.
    struct CountingFilter(Arc<AtomicUsize>);

    impl RecordFilter for CountingFilter {
        fn check(&self, _record: &Record) -> bool {
            self.0.fetch_add(1, Ordering::SeqCst);
            true
        }
    }

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
    fn test_check_raw_defaults_to_formatted_check() {
        let formatter = CountingFormatter(Cell::new(0));
        let filter = MessageFilter("ff");

        assert!(filter.check_raw(&RawRecord::new(RecordKind::Read, &[0x0a, 0xff], &formatter)));
        assert!(!filter.check_raw(&RawRecord::new(
            RecordKind::Write,
            &[0x0a, 0x0b],
            &formatter
        )));
        // The default has to format each record to hand it to `check`.
        assert_eq!(formatter.0.get(), 2);
    }

    #[test]
    fn test_provided_filters_check_raw_agrees_with_check_without_formatting() {
        let kinds = [
            RecordKind::Open,
            RecordKind::Read,
            RecordKind::Write,
            RecordKind::Error,
            RecordKind::Shutdown,
            RecordKind::Drop,
        ];
        let filters: Vec<Box<dyn RecordFilter>> = vec![
            Box::new(DefaultFilter),
            Box::new(RecordKindFilter::new(&[])),
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(RecordKindFilter::new(&[
                RecordKind::Drop,
                RecordKind::Error,
                RecordKind::Shutdown,
            ])),
            Box::new(AllFilter::new(vec![])),
            Box::new(AnyFilter::new(vec![])),
            Box::new(AllFilter::new(vec![
                Box::new(RecordKindFilter::new(&[
                    RecordKind::Read,
                    RecordKind::Write,
                ])),
                Box::new(RecordKindFilter::new(&[
                    RecordKind::Read,
                    RecordKind::Error,
                ])),
            ])),
            Box::new(AnyFilter::new(vec![
                Box::new(RecordKindFilter::new(&[RecordKind::Read])),
                Box::new(RecordKindFilter::new(&[RecordKind::Write])),
            ])),
            Box::new(AllFilter::new(vec![Box::new(AnyFilter::new(vec![
                Box::new(DefaultFilter),
                Box::new(RecordKindFilter::new(&[RecordKind::Write])),
            ]))])),
        ];
        let formatter = CountingFormatter(Cell::new(0));
        let reference = LowercaseHexadecimalFormatter::new_default();

        for filter in &filters {
            for kind in kinds {
                let raw = RawRecord::new(kind, &[0x01, 0x02], &formatter);
                let record = Record::new(kind, reference.format_buffer(&[0x01, 0x02]));
                assert_eq!(
                    filter.check_raw(&raw),
                    filter.check(&record),
                    "{:?} disagrees for {kind:?}",
                    RecordFilterDebug(filter.as_ref())
                );
            }
        }
        // The provided filters decide from the kind alone, so nothing was formatted.
        assert_eq!(formatter.0.get(), 0);
    }

    #[test]
    fn test_box_forwards_check_raw() {
        let formatter = LowercaseHexadecimalFormatter::new_default();
        let handshake = RawRecord::new(RecordKind::Read, &[0x16, 0x03], &formatter);
        let application_data = RawRecord::new(RecordKind::Read, &[0x17, 0x03], &formatter);

        // `check` keeps everything, so only a forwarded `check_raw` can reject the application data.
        let boxed: Box<dyn RecordFilter> = Box::new(FirstByteFilter(0x16));
        assert!(boxed.check_raw(&handshake));
        assert!(!boxed.check_raw(&application_data));

        let double_boxed: Box<Box<dyn RecordFilter>> = Box::new(boxed);
        assert!(!double_boxed.check_raw(&application_data));

        // Composite children are boxed too.
        let all = AllFilter::new(vec![Box::new(FirstByteFilter(0x16))]);
        assert!(!all.check_raw(&application_data));
        let any = AnyFilter::new(vec![Box::new(FirstByteFilter(0x16))]);
        assert!(!any.check_raw(&application_data));
    }

    #[test]
    fn test_composite_check_raw_formats_once_for_several_children() {
        let formatter = CountingFormatter(Cell::new(0));
        let raw = RawRecord::new(RecordKind::Read, &[0x0a, 0xff], &formatter);

        let filter = AllFilter::new(vec![
            Box::new(MessageFilter("0a")),
            Box::new(MessageFilter("ff")),
        ]);
        assert!(filter.check_raw(&raw));
        assert_eq!(formatter.0.get(), 1);
    }

    #[test]
    fn test_composite_check_raw_keeps_child_order() {
        // A child that needs the formatted record comes before a kind filter: it still sees the write, exactly
        // as `check` would show it.
        let calls = Arc::new(AtomicUsize::new(0));
        let formatter = CountingFormatter(Cell::new(0));
        let write = RawRecord::new(RecordKind::Write, &[0x01], &formatter);
        let filter = AllFilter::new(vec![
            Box::new(CountingFilter(Arc::clone(&calls))),
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
        ]);
        assert!(!filter.check_raw(&write));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(formatter.0.get(), 1);

        // The kind filter comes first: the write is rejected before the child or the formatter runs, exactly as
        // `check` would stop at the kind filter.
        let calls = Arc::new(AtomicUsize::new(0));
        let formatter = CountingFormatter(Cell::new(0));
        let write = RawRecord::new(RecordKind::Write, &[0x01], &formatter);
        let filter = AllFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(CountingFilter(Arc::clone(&calls))),
        ]);
        assert!(!filter.check_raw(&write));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(formatter.0.get(), 0);

        // AnyFilter mirrors it: a kind filter that accepts settles the read before the child runs.
        let calls = Arc::new(AtomicUsize::new(0));
        let formatter = CountingFormatter(Cell::new(0));
        let read = RawRecord::new(RecordKind::Read, &[0x01], &formatter);
        let filter = AnyFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(CountingFilter(Arc::clone(&calls))),
        ]);
        assert!(filter.check_raw(&read));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(formatter.0.get(), 0);
    }

    #[test]
    fn test_composite_lets_raw_child_decide_after_check_only_child() {
        // The first child judges the formatted message and accepts; the raw child after it still decides on the
        // raw bytes, although its `check` would keep everything. Each record is formatted once, for the first
        // child.
        let formatter = CountingFormatter(Cell::new(0));
        let all = AllFilter::new(vec![
            Box::new(MessageFilter("16")),
            Box::new(FirstByteFilter(0x16)),
        ]);
        assert!(all.check_raw(&RawRecord::new(RecordKind::Read, &[0x16, 0x01], &formatter)));
        assert!(!all.check_raw(&RawRecord::new(RecordKind::Read, &[0x01, 0x16], &formatter)));
        assert_eq!(formatter.0.get(), 2);

        // AnyFilter mirrors it: the first child rejects on the formatted message, the raw child decides.
        let formatter = CountingFormatter(Cell::new(0));
        let any = AnyFilter::new(vec![
            Box::new(MessageFilter("ff")),
            Box::new(FirstByteFilter(0x16)),
        ]);
        assert!(any.check_raw(&RawRecord::new(RecordKind::Read, &[0x16, 0x01], &formatter)));
        assert!(!any.check_raw(&RawRecord::new(RecordKind::Read, &[0x01, 0x02], &formatter)));
        assert_eq!(formatter.0.get(), 2);
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

        let formatter = LowercaseHexadecimalFormatter::new_default();
        let raw = RawRecord::new(RecordKind::Read, &[0x01], &formatter);
        assert!(default.check_raw(&raw));
        assert!(!record_kind.check_raw(&raw));
        assert!(all.check_raw(&raw));
        assert!(!any.check_raw(&raw));
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
