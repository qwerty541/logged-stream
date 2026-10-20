use crate::Event;
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
/// [`LoggedStream`] calls [`check`] for every event (every read, write, error, shutdown, drop and `Open` marker)
/// and logs the event's record only if it returns `true`.
///
/// # Formatted or raw
///
/// [`check`] receives an [`Event`], which offers three views of what happened; a filter uses whichever it needs:
///
/// -   [`Event::record`]: the [`Record`] exactly as it would be logged. For a read or write, that is its bytes
///     rendered by the stream's formatter into the human-readable text you see in the log; for any other event, its
///     message. Custom filters are usually written against what the log shows, such as "keep the writes that contain
///     `ff:ff`", and matching that text is much easier than decoding raw bytes, so this is the simplest way to write
///     a filter.
/// -   [`Event::bytes`]: the raw bytes of a read or write, exactly as they passed through the stream, and `None` for
///     any other event. Use them for what the formatted text obscures, such as a binary protocol header.
/// -   [`Event::kind`]: the [`RecordKind`].
///
/// The views differ in cost. [`LoggedStream`] formats a read or write only when a filter asks for its record or when
/// the record is logged, and at most once: the filters and the logger share one record. Formatting is by far the
/// most expensive step of logging a read or write (the provided formatters allocate once per byte), so one that is
/// rejected without anyone asking for its record costs almost nothing. Measured with `cargo bench --bench stream` on
/// a laptop (Intel Core i5-8265U), rejecting an 8 KiB read took about 0.72 ms when it was formatted first and about
/// 0.09 µs when it was not, about the cost of the read itself; for a 64-byte read, about 6 µs and 4 ns. A filter
/// that rejects most reads and writes of a busy stream should therefore decide on [`Event::kind`] or
/// [`Event::bytes`] where it can, and ask for [`Event::record`] only for the rest. [`DefaultFilter`] and
/// [`RecordKindFilter`] never ask for it.
///
/// A filter that asks for [`Event::record`] for every event has every read and write formatted, which is what a
/// formatter with side effects that must see every buffer needs.
///
/// # Examples
///
/// Two filters that keep the reads and writes starting with a TLS handshake byte (0x16), and every other event. The
/// first judges the formatted record, the second the raw bytes, so the reads and writes it rejects are never
/// formatted:
///
/// ```
/// use logged_stream::Event;
/// use logged_stream::LoggedStream;
/// use logged_stream::LowercaseHexadecimalFormatter;
/// use logged_stream::MemoryStorageLogger;
/// use logged_stream::RecordFilter;
/// use logged_stream::RecordKind;
/// use std::io::Cursor;
/// use std::io::Read;
///
/// struct TlsHandshakes;
///
/// impl RecordFilter for TlsHandshakes {
///     fn check(&self, event: &Event<'_>) -> bool {
///         let record = event.record();
///         match record.kind {
///             RecordKind::Read | RecordKind::Write => record.message.starts_with("16"),
///             _ => true,
///         }
///     }
/// }
///
/// struct TlsHandshakesRaw;
///
/// impl RecordFilter for TlsHandshakesRaw {
///     fn check(&self, event: &Event<'_>) -> bool {
///         match event.bytes() {
///             Some(bytes) => bytes.first() == Some(&0x16),
///             None => true,
///         }
///     }
/// }
///
/// let filters: [Box<dyn RecordFilter>; 2] = [Box::new(TlsHandshakes), Box::new(TlsHandshakesRaw)];
/// for filter in filters {
///     let mut stream = LoggedStream::new(
///         Cursor::new(vec![0x16, 0x03, 0x01, 0x17, 0x03]),
///         LowercaseHexadecimalFormatter::new_default(),
///         filter,
///         MemoryStorageLogger::new(16),
///     );
///
///     let mut handshake = [0u8; 3];
///     stream.read_exact(&mut handshake)?; // kept
///     let mut application_data = [0u8; 2];
///     stream.read_exact(&mut application_data)?; // rejected
///
///     let records = stream.get_log_records();
///     assert_eq!(records.len(), 1);
///     assert_eq!(records[0].message, "16:03:01");
/// }
/// # Ok::<(), std::io::Error>(())
/// ```
///
/// The [`custom-filters`] example in the repository runs two such filters side by side and counts how many buffers
/// each has formatted.
///
/// # Wrapping other filters
///
/// A filter that wraps or combines others should hand them the [`Event`] it was given, as `Box`, [`AllFilter`] and
/// [`AnyFilter`] do, so that a read or write is formatted at most once however many of them ask for its record.
///
/// [`check`]: RecordFilter::check
/// [`LoggedStream`]: crate::LoggedStream
/// [`Record`]: crate::Record
/// [`custom-filters`]: https://github.com/qwerty541/logged-stream/blob/master/examples/custom-filters.rs
pub trait RecordFilter: Send + 'static {
    /// This method returns [`bool`] value depending on if received event ([`Event`]) should be processed by logging
    /// part inside [`LoggedStream`].
    ///
    /// [`LoggedStream`] calls it for every event, of every [`RecordKind`]. The [trait documentation](RecordFilter)
    /// describes what an [`Event`] offers and what each of its views costs.
    ///
    /// [`LoggedStream`]: crate::LoggedStream
    fn check(&self, event: &Event<'_>) -> bool;

    /// This method provides [`fmt::Debug`] representation of the filter. It is used by composite filters
    /// ([`AllFilter`] and [`AnyFilter`]) to produce detailed debug output of their underlying filters.
    /// The default implementation outputs the type name as `"UnknownFilter"`.
    /// Implementors are encouraged to override this method to provide meaningful debug information.
    fn fmt_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UnknownFilter")
    }
}

impl<T: RecordFilter + ?Sized> RecordFilter for Box<T> {
    fn check(&self, event: &Event<'_>) -> bool {
        (**self).check(event)
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
    fn check(&self, _event: &Event<'_>) -> bool {
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
/// It decides from [`Event::kind`] alone and never asks for the formatted record, so on its own it rejects reads
/// and writes without formatting them.
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
    fn check(&self, event: &Event<'_>) -> bool {
        self.allowed_kinds.contains(&event.kind())
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
/// The underlying filters are asked in order, and the first one that returns `false` settles the result. They all
/// receive the same [`Event`], so a read or write is formatted at most once however many of them ask for its record.
///
/// This filter is useful for combining multiple filtering conditions where all must be satisfied.
///
/// # Examples
///
/// ```
/// use logged_stream::AllFilter;
/// use logged_stream::Event;
/// use logged_stream::LowercaseHexadecimalFormatter;
/// use logged_stream::Record;
/// use logged_stream::RecordFilter;
/// use logged_stream::RecordKind;
/// use logged_stream::RecordKindFilter;
///
/// // Create a filter that accepts only Read operations
/// let filter = AllFilter::new(vec![
///     Box::new(RecordKindFilter::new(&[RecordKind::Read])),
/// ]);
///
/// let formatter = LowercaseHexadecimalFormatter::new_default();
/// assert!(filter.check(&Event::read(b"data", &formatter)));
///
/// let error_record = Record::new(RecordKind::Error, String::from("error"));
/// assert!(!filter.check(&Event::from(error_record)));
/// ```
///
/// [`check`]: RecordFilter::check
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
    /// use logged_stream::AllFilter;
    /// use logged_stream::RecordKind;
    /// use logged_stream::RecordKindFilter;
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
    fn check(&self, event: &Event<'_>) -> bool {
        self.filters.iter().all(|filter| filter.check(event))
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
/// The underlying filters are asked in order, and the first one that returns `true` settles the result. They all
/// receive the same [`Event`], so a read or write is formatted at most once however many of them ask for its record.
///
/// This filter is useful for combining multiple filtering conditions where at least one must be satisfied.
///
/// # Examples
///
/// ```
/// use logged_stream::AnyFilter;
/// use logged_stream::Event;
/// use logged_stream::LowercaseHexadecimalFormatter;
/// use logged_stream::Record;
/// use logged_stream::RecordFilter;
/// use logged_stream::RecordKind;
/// use logged_stream::RecordKindFilter;
///
/// // Create a filter that accepts Read OR Write operations
/// let filter = AnyFilter::new(vec![
///     Box::new(RecordKindFilter::new(&[RecordKind::Read])),
///     Box::new(RecordKindFilter::new(&[RecordKind::Write])),
/// ]);
///
/// let formatter = LowercaseHexadecimalFormatter::new_default();
/// assert!(filter.check(&Event::read(b"data", &formatter)));
/// assert!(filter.check(&Event::write(b"data", &formatter)));
///
/// let error_record = Record::new(RecordKind::Error, String::from("error"));
/// assert!(!filter.check(&Event::from(error_record)));
/// ```
///
/// [`check`]: RecordFilter::check
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
    /// use logged_stream::AnyFilter;
    /// use logged_stream::RecordKind;
    /// use logged_stream::RecordKindFilter;
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
    fn check(&self, event: &Event<'_>) -> bool {
        self.filters.iter().any(|filter| filter.check(event))
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
    use crate::record::Event;
    use crate::record::Record;
    use crate::record::RecordKind;
    use std::cell::Cell;
    use std::fmt;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    /// An event without bytes that carries `message`, like the ones `LoggedStream` builds for errors, shutdowns,
    /// drops and opens. The provided filters judge only its kind.
    fn event(kind: RecordKind, message: &str) -> Event<'static> {
        Event::from(Record::new(kind, String::from(message)))
    }

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

    /// Judges the formatted record: keeps events whose message contains the needle.
    struct MessageFilter(&'static str);

    impl RecordFilter for MessageFilter {
        fn check(&self, event: &Event<'_>) -> bool {
            event.record().message.contains(self.0)
        }
    }

    /// Judges reads and writes on their first raw byte, and keeps every other event.
    struct FirstByteFilter(u8);

    impl RecordFilter for FirstByteFilter {
        fn check(&self, event: &Event<'_>) -> bool {
            match event.bytes() {
                Some(bytes) => bytes.first() == Some(&self.0),
                None => true,
            }
        }
    }

    /// Asks for the formatted record of every event, keeps the event, and counts its calls.
    struct CountingFilter(Arc<AtomicUsize>);

    impl RecordFilter for CountingFilter {
        fn check(&self, event: &Event<'_>) -> bool {
            self.0.fetch_add(1, Ordering::SeqCst);
            event.record();
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
        assert!(DefaultFilter.check(&event(RecordKind::Read, "01:02:03:04:05:06")));
        assert!(DefaultFilter.check(&event(RecordKind::Write, "01:02:03:04:05:06")));
        assert!(DefaultFilter.check(&event(RecordKind::Drop, "deallocated")));
        assert!(DefaultFilter.check(&event(RecordKind::Shutdown, "write shutdown request")));
    }

    #[test]
    fn test_record_kind_filter() {
        let filter = RecordKindFilter::new(&[RecordKind::Read]);
        assert!(filter.check(&event(RecordKind::Read, "01:02:03:04:05:06")));
        assert!(!filter.check(&event(RecordKind::Write, "01:02:03:04:05:06")));
        assert!(!filter.check(&event(RecordKind::Drop, "deallocated")));
        assert!(!filter.check(&event(RecordKind::Shutdown, "write shutdown request")));
    }

    #[test]
    fn test_all_filter_empty() {
        let filter = AllFilter::new(vec![]);
        // Empty conjunction should return true (vacuously true)
        assert!(filter.check(&event(RecordKind::Read, "test")));
        assert!(filter.check(&event(RecordKind::Write, "test")));
        assert!(filter.check(&event(RecordKind::Error, "test")));
    }

    #[test]
    fn test_all_filter_single() {
        let filter = AllFilter::new(vec![Box::new(RecordKindFilter::new(&[RecordKind::Read]))]);

        assert!(filter.check(&event(RecordKind::Read, "data")));
        assert!(!filter.check(&event(RecordKind::Write, "data")));
        assert!(!filter.check(&event(RecordKind::Error, "error")));
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

        assert!(filter.check(&event(RecordKind::Read, "data")));
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

        assert!(!filter.check(&event(RecordKind::Write, "data")));
    }

    #[test]
    fn test_all_filter_multiple_all_fail() {
        let filter = AllFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(RecordKindFilter::new(&[RecordKind::Write])),
        ]);

        assert!(!filter.check(&event(RecordKind::Error, "error")));
    }

    #[test]
    fn test_all_filter_with_default() {
        // Combining with DefaultFilter (which always returns true)
        let filter = AllFilter::new(vec![
            Box::new(DefaultFilter),
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
        ]);

        assert!(filter.check(&event(RecordKind::Read, "data")));
        assert!(!filter.check(&event(RecordKind::Write, "data")));
    }

    #[test]
    fn test_any_filter_empty() {
        let filter = AnyFilter::new(vec![]);
        // Empty disjunction should return false
        assert!(!filter.check(&event(RecordKind::Read, "test")));
        assert!(!filter.check(&event(RecordKind::Write, "test")));
        assert!(!filter.check(&event(RecordKind::Error, "test")));
    }

    #[test]
    fn test_any_filter_single() {
        let filter = AnyFilter::new(vec![Box::new(RecordKindFilter::new(&[RecordKind::Read]))]);

        assert!(filter.check(&event(RecordKind::Read, "data")));
        assert!(!filter.check(&event(RecordKind::Write, "data")));
        assert!(!filter.check(&event(RecordKind::Error, "error")));
    }

    #[test]
    fn test_any_filter_multiple_first_passes() {
        let filter = AnyFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(RecordKindFilter::new(&[RecordKind::Write])),
        ]);

        assert!(filter.check(&event(RecordKind::Read, "data")));
    }

    #[test]
    fn test_any_filter_multiple_second_passes() {
        let filter = AnyFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(RecordKindFilter::new(&[RecordKind::Write])),
        ]);

        assert!(filter.check(&event(RecordKind::Write, "data")));
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

        assert!(filter.check(&event(RecordKind::Read, "data")));
    }

    #[test]
    fn test_any_filter_multiple_all_fail() {
        let filter = AnyFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(RecordKindFilter::new(&[RecordKind::Write])),
        ]);

        assert!(!filter.check(&event(RecordKind::Error, "error")));
    }

    #[test]
    fn test_any_filter_with_default() {
        // Combining with DefaultFilter (which always returns true)
        let filter = AnyFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(DefaultFilter),
        ]);

        // Should pass for everything because DefaultFilter always returns true
        assert!(filter.check(&event(RecordKind::Read, "data")));
        assert!(filter.check(&event(RecordKind::Write, "data")));
        assert!(filter.check(&event(RecordKind::Error, "error")));
    }

    #[test]
    fn test_nested_composite_filters() {
        // (Read OR Write)
        // Implemented as: AllFilter containing AnyFilter for (Read OR Write)
        let filter = AllFilter::new(vec![Box::new(AnyFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(RecordKindFilter::new(&[RecordKind::Write])),
        ]))]);

        assert!(filter.check(&event(RecordKind::Read, "data")));
        assert!(filter.check(&event(RecordKind::Write, "data")));
        assert!(!filter.check(&event(RecordKind::Drop, "dropped")));
        assert!(!filter.check(&event(RecordKind::Error, "error")));
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

        assert!(filter.check(&event(RecordKind::Read, "data")));
        assert!(!filter.check(&event(RecordKind::Write, "data")));
        assert!(!filter.check(&event(RecordKind::Error, "error")));
        assert!(!filter.check(&event(RecordKind::Drop, "dropped")));
    }

    #[test]
    fn test_provided_filters_decide_by_kind_without_formatting() {
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

        for filter in &filters {
            for kind in kinds {
                // The event `LoggedStream` builds: bytes for a read or write, a message for anything else.
                let built = match kind {
                    RecordKind::Read => Event::read(&[0x01, 0x02], &formatter),
                    RecordKind::Write => Event::write(&[0x01, 0x02], &formatter),
                    _ => event(kind, "message"),
                };
                assert_eq!(
                    filter.check(&built),
                    filter.check(&event(kind, "01:02")),
                    "{:?} judges more than the kind of {kind:?}",
                    RecordFilterDebug(filter.as_ref())
                );
            }
        }
        // They decide from the kind alone, so nothing was formatted.
        assert_eq!(formatter.0.get(), 0);
    }

    #[test]
    fn test_box_forwards_check() {
        let formatter = LowercaseHexadecimalFormatter::new_default();
        let handshake = Event::read(&[0x16, 0x03], &formatter);
        let application_data = Event::read(&[0x17, 0x03], &formatter);

        // The filter decides on the raw bytes, so each wrapper has to hand on the event it was given.
        let boxed: Box<dyn RecordFilter> = Box::new(FirstByteFilter(0x16));
        assert!(boxed.check(&handshake));
        assert!(!boxed.check(&application_data));

        let double_boxed: Box<Box<dyn RecordFilter>> = Box::new(boxed);
        assert!(!double_boxed.check(&application_data));

        // Composite children are boxed too.
        let all = AllFilter::new(vec![Box::new(FirstByteFilter(0x16))]);
        assert!(!all.check(&application_data));
        let any = AnyFilter::new(vec![Box::new(FirstByteFilter(0x16))]);
        assert!(!any.check(&application_data));
    }

    #[test]
    fn test_composite_formats_once_for_several_children() {
        let formatter = CountingFormatter(Cell::new(0));
        let read = Event::read(&[0x0a, 0xff], &formatter);

        let filter = AllFilter::new(vec![
            Box::new(MessageFilter("0a")),
            Box::new(MessageFilter("ff")),
        ]);
        assert!(filter.check(&read));
        assert_eq!(formatter.0.get(), 1);
    }

    #[test]
    fn test_composite_keeps_child_order() {
        // A child that asks for the formatted record comes before a kind filter: it still sees the write, and the
        // write is formatted for it.
        let calls = Arc::new(AtomicUsize::new(0));
        let formatter = CountingFormatter(Cell::new(0));
        let write = Event::write(&[0x01], &formatter);
        let filter = AllFilter::new(vec![
            Box::new(CountingFilter(Arc::clone(&calls))),
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
        ]);
        assert!(!filter.check(&write));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(formatter.0.get(), 1);

        // The kind filter comes first: the write is rejected before the child or the formatter runs.
        let calls = Arc::new(AtomicUsize::new(0));
        let formatter = CountingFormatter(Cell::new(0));
        let write = Event::write(&[0x01], &formatter);
        let filter = AllFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(CountingFilter(Arc::clone(&calls))),
        ]);
        assert!(!filter.check(&write));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(formatter.0.get(), 0);

        // AnyFilter mirrors it: a kind filter that accepts settles the read before the child runs.
        let calls = Arc::new(AtomicUsize::new(0));
        let formatter = CountingFormatter(Cell::new(0));
        let read = Event::read(&[0x01], &formatter);
        let filter = AnyFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(CountingFilter(Arc::clone(&calls))),
        ]);
        assert!(filter.check(&read));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(formatter.0.get(), 0);
    }

    #[test]
    fn test_composite_mixes_formatted_and_raw_children() {
        // The first child judges the formatted message and accepts; the raw child after it still decides on the
        // raw bytes. Each event is formatted once, for the first child.
        let formatter = CountingFormatter(Cell::new(0));
        let all = AllFilter::new(vec![
            Box::new(MessageFilter("16")),
            Box::new(FirstByteFilter(0x16)),
        ]);
        assert!(all.check(&Event::read(&[0x16, 0x01], &formatter)));
        assert!(!all.check(&Event::read(&[0x01, 0x16], &formatter)));
        assert_eq!(formatter.0.get(), 2);

        // AnyFilter mirrors it: the first child rejects on the formatted message, the raw child decides.
        let formatter = CountingFormatter(Cell::new(0));
        let any = AnyFilter::new(vec![
            Box::new(MessageFilter("ff")),
            Box::new(FirstByteFilter(0x16)),
        ]);
        assert!(any.check(&Event::read(&[0x16, 0x01], &formatter)));
        assert!(!any.check(&Event::read(&[0x01, 0x02], &formatter)));
        assert_eq!(formatter.0.get(), 2);
    }

    #[test]
    fn test_trait_object_safety() {
        // Assert trait object construct.
        let default: Box<dyn RecordFilter> = Box::<DefaultFilter>::default();
        let record_kind: Box<dyn RecordFilter> = Box::new(RecordKindFilter::new(&[]));
        let all: Box<dyn RecordFilter> = Box::new(AllFilter::new(vec![]));
        let any: Box<dyn RecordFilter> = Box::new(AnyFilter::new(vec![]));

        let open = event(RecordKind::Open, "test log record");

        // Assert that trait object methods are dispatchable.
        _ = default.check(&open);
        _ = record_kind.check(&open);
        _ = all.check(&open);
        _ = any.check(&open);

        let formatter = LowercaseHexadecimalFormatter::new_default();
        let read = Event::read(&[0x01], &formatter);
        assert!(default.check(&read));
        assert!(!record_kind.check(&read));
        assert!(all.check(&read));
        assert!(!any.check(&read));
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
