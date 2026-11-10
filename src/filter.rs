use crate::Event;
use crate::EventKind;
use itertools::Itertools;
use std::fmt;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Trait
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Trait for filtering events in [`LoggedStream`].
///
/// This trait allows filtering events ([`Event`]) using the [`check`] method, which returns a [`bool`] value.
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
/// -   [`Event::kind`]: the [`EventKind`].
///
/// The views differ in cost. [`LoggedStream`] formats a read or write only when a filter asks for its record or when
/// the record is logged, and at most once: the filters and the logger share one record. Formatting is by far the
/// most expensive step of logging a read or write (the provided formatters allocate once per byte), so one that is
/// rejected without anyone asking for its record costs almost nothing. Measured with `cargo bench --bench stream` on
/// a laptop (Intel Core i5-8265U), rejecting an 8 KiB read took about 0.72 ms when it was formatted first and about
/// 0.09 µs when it was not, about the cost of the read itself; for a 64-byte read, about 6 µs and 4 ns. A filter
/// that rejects most reads and writes of a busy stream should therefore decide on [`Event::kind`] or
/// [`Event::bytes`] where it can, and ask for [`Event::record`] only for the rest. [`DefaultFilter`] and
/// [`EventKindFilter`] never ask for it.
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
/// use logged_stream::EventFilter;
/// use logged_stream::EventKind;
/// use logged_stream::LoggedStream;
/// use logged_stream::LowercaseHexadecimalFormatter;
/// use logged_stream::MemoryStorageLogger;
/// use std::io::Cursor;
/// use std::io::Read;
///
/// struct TlsHandshakes;
///
/// impl EventFilter for TlsHandshakes {
///     fn check(&self, event: &Event<'_>) -> bool {
///         let record = event.record();
///         match record.kind {
///             EventKind::Read | EventKind::Write => record.message.starts_with("16"),
///             _ => true,
///         }
///     }
/// }
///
/// struct TlsHandshakesRaw;
///
/// impl EventFilter for TlsHandshakesRaw {
///     fn check(&self, event: &Event<'_>) -> bool {
///         match event.bytes() {
///             Some(bytes) => bytes.first() == Some(&0x16),
///             None => true,
///         }
///     }
/// }
///
/// let filters: [Box<dyn EventFilter>; 2] = [Box::new(TlsHandshakes), Box::new(TlsHandshakesRaw)];
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
/// [`check`]: EventFilter::check
/// [`LoggedStream`]: crate::LoggedStream
/// [`Record`]: crate::Record
/// [`custom-filters`]: https://github.com/qwerty541/logged-stream/blob/master/examples/custom-filters.rs
pub trait EventFilter: Send + 'static {
    /// This method returns [`bool`] value depending on if received event ([`Event`]) should be processed by logging
    /// part inside [`LoggedStream`].
    ///
    /// [`LoggedStream`] calls it for every event, of every [`EventKind`]. The [trait documentation](EventFilter)
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

impl<T: EventFilter + ?Sized> EventFilter for Box<T> {
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

/// This is default implementation of [`EventFilter`] trait which [`check`] method always return `true`.
/// It should be constructed using [`Default::default`] method.
///
/// [`check`]: EventFilter::check
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultFilter;

impl EventFilter for DefaultFilter {
    #[inline]
    fn check(&self, _event: &Event<'_>) -> bool {
        true
    }

    fn fmt_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// EventKindFilter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Implementation of [`EventFilter`] that accepts allowed [`EventKind`] array.
///
/// This implementation of the [`EventFilter`] trait accepts an array of allowed event kinds ([`EventKind`]) during
/// construction. Its [`check`] method returns `true` if the kind of the received event is present in this array.
/// It decides from [`Event::kind`] alone and never asks for the formatted record, so on its own it rejects reads
/// and writes without formatting them.
///
/// [`check`]: EventFilter::check
#[derive(Debug)]
pub struct EventKindFilter {
    allowed_kinds: Vec<EventKind>,
}

impl EventKindFilter {
    /// Construct a new instance of [`EventKindFilter`] using provided array of allowed event kinds ([`EventKind`]).
    pub fn new(kinds: &'static [EventKind]) -> Self {
        Self {
            allowed_kinds: kinds.iter().copied().unique().collect(),
        }
    }
}

impl EventFilter for EventKindFilter {
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

/// Implementation of [`EventFilter`] that combines multiple filters with AND logic.
///
/// This implementation of the [`EventFilter`] trait accepts a vector of boxed filters during construction.
/// Its [`check`] method returns `true` only if **all** underlying filters return `true` for the given event.
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
/// use logged_stream::EventFilter;
/// use logged_stream::EventKind;
/// use logged_stream::EventKindFilter;
/// use logged_stream::LowercaseHexadecimalFormatter;
///
/// // Create a filter that accepts only Read operations
/// let filter = AllFilter::new(vec![
///     Box::new(EventKindFilter::new(&[EventKind::Read])),
/// ]);
///
/// let formatter = LowercaseHexadecimalFormatter::new_default();
/// assert!(filter.check(&Event::read(b"data", &formatter)));
/// assert!(!filter.check(&Event::message(EventKind::Error, "error")));
/// ```
///
/// [`check`]: EventFilter::check
pub struct AllFilter {
    filters: Vec<Box<dyn EventFilter>>,
}

impl fmt::Debug for AllFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AllFilter")
            .field("filters", &EventFiltersDebug(&self.filters))
            .finish()
    }
}

impl AllFilter {
    /// Construct a new instance of [`AllFilter`] using provided vector of boxed filters.
    ///
    /// # Arguments
    ///
    /// * `filters` - A vector of boxed filters implementing [`EventFilter`] trait
    ///
    /// # Examples
    ///
    /// ```
    /// use logged_stream::AllFilter;
    /// use logged_stream::EventKind;
    /// use logged_stream::EventKindFilter;
    ///
    /// let filter = AllFilter::new(vec![
    ///     Box::new(EventKindFilter::new(&[EventKind::Read])),
    ///     Box::new(EventKindFilter::new(&[EventKind::Read, EventKind::Write])),
    /// ]);
    /// ```
    pub fn new(filters: Vec<Box<dyn EventFilter>>) -> Self {
        Self { filters }
    }
}

impl EventFilter for AllFilter {
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

/// Implementation of [`EventFilter`] that combines multiple filters with OR logic.
///
/// This implementation of the [`EventFilter`] trait accepts a vector of boxed filters during construction.
/// Its [`check`] method returns `true` if **any** of the underlying filters returns `true` for the given event.
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
/// use logged_stream::EventFilter;
/// use logged_stream::EventKind;
/// use logged_stream::EventKindFilter;
/// use logged_stream::LowercaseHexadecimalFormatter;
///
/// // Create a filter that accepts Read OR Write operations
/// let filter = AnyFilter::new(vec![
///     Box::new(EventKindFilter::new(&[EventKind::Read])),
///     Box::new(EventKindFilter::new(&[EventKind::Write])),
/// ]);
///
/// let formatter = LowercaseHexadecimalFormatter::new_default();
/// assert!(filter.check(&Event::read(b"data", &formatter)));
/// assert!(filter.check(&Event::write(b"data", &formatter)));
/// assert!(!filter.check(&Event::message(EventKind::Error, "error")));
/// ```
///
/// [`check`]: EventFilter::check
pub struct AnyFilter {
    filters: Vec<Box<dyn EventFilter>>,
}

impl fmt::Debug for AnyFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnyFilter")
            .field("filters", &EventFiltersDebug(&self.filters))
            .finish()
    }
}

impl AnyFilter {
    /// Construct a new instance of [`AnyFilter`] using provided vector of boxed filters.
    ///
    /// # Arguments
    ///
    /// * `filters` - A vector of boxed filters implementing [`EventFilter`] trait
    ///
    /// # Examples
    ///
    /// ```
    /// use logged_stream::AnyFilter;
    /// use logged_stream::EventKind;
    /// use logged_stream::EventKindFilter;
    ///
    /// let filter = AnyFilter::new(vec![
    ///     Box::new(EventKindFilter::new(&[EventKind::Read])),
    ///     Box::new(EventKindFilter::new(&[EventKind::Write])),
    /// ]);
    /// ```
    pub fn new(filters: Vec<Box<dyn EventFilter>>) -> Self {
        Self { filters }
    }
}

impl EventFilter for AnyFilter {
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

/// Helper wrapper to bridge [`EventFilter::fmt_debug`] into [`fmt::Debug`].
///
/// It is `pub(crate)`, not private, because the filter tests in `crate::tests` use it to print a filter
/// in an assertion message.
pub(crate) struct EventFilterDebug<'a>(pub(crate) &'a dyn EventFilter);

impl fmt::Debug for EventFilterDebug<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt_debug(f)
    }
}

/// Helper wrapper to format a slice of [`EventFilter`]s without allocating.
struct EventFiltersDebug<'a>(&'a [Box<dyn EventFilter>]);

impl fmt::Debug for EventFiltersDebug<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut list = f.debug_list();
        for filter in self.0.iter() {
            list.entry(&EventFilterDebug(filter.as_ref()));
        }
        list.finish()
    }
}
