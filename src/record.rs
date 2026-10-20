use crate::BufferFormatter;
use chrono::DateTime;
use chrono::Utc;
use std::cell::OnceCell;
use std::fmt;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Record
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// This structure represents a log record and contains message string, creation timestamp ([`DateTime`]<[`Utc`]>)
/// and record kind ([`RecordKind`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Record {
    pub kind: RecordKind,
    pub message: String,
    pub time: DateTime<Utc>,
}

impl Record {
    /// Construct a new instance of [`Record`] using provided message and kind.
    pub fn new(kind: RecordKind, message: String) -> Self {
        Self {
            kind,
            message,
            time: Utc::now(),
        }
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Event
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// One event of a [`LoggedStream`], as its filter sees it: [`RecordFilter::check`] receives it and decides
/// whether its record is logged.
///
/// Every event has a [kind](Event::kind) and a [record](Event::record), the [`Record`] that the logger receives
/// if the filter accepts the event. A read or write also carries the raw [bytes](Event::bytes) that passed
/// through the stream, and its record is formatted from them the first time it is asked for, then reused: by any
/// other filter that asks for it, and as the record handed to the logger. A read or write is therefore formatted
/// at most once, and one that is rejected before anything asks for its record is not formatted at all. Every
/// other event (an error, shutdown, drop or `Open` marker) carries its record from the start.
///
/// [`LoggedStream`] builds the events itself. To test a filter of your own, build them with [`Event::read`],
/// [`Event::write`] or, for an event without bytes, from a [`Record`]:
///
/// ```
/// use logged_stream::Event;
/// use logged_stream::LowercaseHexadecimalFormatter;
/// use logged_stream::Record;
/// use logged_stream::RecordKind;
///
/// let formatter = LowercaseHexadecimalFormatter::new_default();
/// let read = Event::read(&[0x16, 0x03, 0x01], &formatter);
/// assert_eq!(read.kind(), RecordKind::Read);
/// assert_eq!(read.bytes(), Some(&[0x16, 0x03, 0x01][..]));
/// assert_eq!(read.record().message, "16:03:01");
///
/// let deallocated = Event::from(Record::new(RecordKind::Drop, String::from("Deallocated.")));
/// assert_eq!(deallocated.kind(), RecordKind::Drop);
/// assert_eq!(deallocated.bytes(), None);
/// assert_eq!(deallocated.record().message, "Deallocated.");
/// ```
///
/// [`LoggedStream`]: crate::LoggedStream
/// [`RecordFilter::check`]: crate::RecordFilter::check
pub struct Event<'a>(Inner<'a>);

enum Inner<'a> {
    /// A read or write: its bytes, formatted into its record the first time the record is asked for.
    Bytes {
        kind: RecordKind,
        bytes: &'a [u8],
        formatter: &'a dyn BufferFormatter,
        record: OnceCell<Record>,
    },
    /// Any other event, whose record exists before the filter runs.
    Message(Record),
}

impl<'a> Event<'a> {
    /// Construct a read of `bytes`, which `formatter` renders into its record when the record is asked for.
    #[inline]
    pub fn read(bytes: &'a [u8], formatter: &'a dyn BufferFormatter) -> Self {
        Self::with_bytes(RecordKind::Read, bytes, formatter)
    }

    /// Construct a write of `bytes`, which `formatter` renders into its record when the record is asked for.
    #[inline]
    pub fn write(bytes: &'a [u8], formatter: &'a dyn BufferFormatter) -> Self {
        Self::with_bytes(RecordKind::Write, bytes, formatter)
    }

    /// Construct a read or write of `bytes`; `kind` is [`RecordKind::Read`] or [`RecordKind::Write`].
    #[inline]
    pub(crate) fn with_bytes(
        kind: RecordKind,
        bytes: &'a [u8],
        formatter: &'a dyn BufferFormatter,
    ) -> Self {
        debug_assert!(matches!(kind, RecordKind::Read | RecordKind::Write));
        Self(Inner::Bytes {
            kind,
            bytes,
            formatter,
            record: OnceCell::new(),
        })
    }

    /// Return the record kind.
    #[inline]
    pub fn kind(&self) -> RecordKind {
        match &self.0 {
            Inner::Bytes { kind, .. } => *kind,
            Inner::Message(record) => record.kind,
        }
    }

    /// Return the bytes of a read or write, exactly as they passed through the stream, or `None` for any other
    /// event. Getting them does not format them.
    ///
    /// Every read and write that [`LoggedStream`] passes to its filter has bytes, possibly empty: a synchronous
    /// stream passes the read that ends it as an empty slice, while an asynchronous read that adds nothing is not
    /// passed to the filter at all. An event built from a [`Record`] has none, whatever its kind.
    ///
    /// [`LoggedStream`]: crate::LoggedStream
    #[inline]
    pub fn bytes(&self) -> Option<&'a [u8]> {
        match &self.0 {
            Inner::Bytes { bytes, .. } => Some(*bytes),
            Inner::Message(_) => None,
        }
    }

    /// Return the record that is logged if the filter accepts the event: for a read or write, its bytes rendered by
    /// the stream's formatter into the text you see in the log; for any other event, its message.
    ///
    /// The first call for a read or write formats the bytes and takes the timestamp; later calls, and the logger,
    /// get the same record. The timestamp is therefore taken when the record is first formatted: if a filter asks
    /// for the record, while the filter decides; otherwise just after the filter accepts the event, when the record
    /// is handed to the logger. Either way it is taken within the same read or write call, after the IO has
    /// completed.
    #[inline]
    pub fn record(&self) -> &Record {
        match &self.0 {
            Inner::Bytes {
                kind,
                bytes,
                formatter,
                record,
            } => record.get_or_init(|| format(*kind, bytes, *formatter)),
            Inner::Message(record) => record,
        }
    }

    /// Consume the event and return its record, formatting it now if nothing asked for it yet.
    #[inline]
    pub(crate) fn into_record(self) -> Record {
        match self.0 {
            Inner::Bytes {
                kind,
                bytes,
                formatter,
                record,
            } => record
                .into_inner()
                .unwrap_or_else(|| format(kind, bytes, formatter)),
            Inner::Message(record) => record,
        }
    }
}

impl From<Record> for Event<'_> {
    /// Construct an event without bytes that carries `record`, as [`LoggedStream`] does for errors, shutdowns,
    /// drops and `Open` markers.
    ///
    /// [`LoggedStream`]: crate::LoggedStream
    #[inline]
    fn from(record: Record) -> Self {
        Self(Inner::Message(record))
    }
}

impl fmt::Debug for Event<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Only a record that already exists is shown, so that debugging an event never formats it.
        let record = match &self.0 {
            Inner::Bytes { record, .. } => record.get(),
            Inner::Message(record) => Some(record),
        };
        f.debug_struct("Event")
            .field("kind", &self.kind())
            .field("bytes", &self.bytes())
            .field("record", &record)
            .finish_non_exhaustive()
    }
}

fn format(kind: RecordKind, bytes: &[u8], formatter: &dyn BufferFormatter) -> Record {
    Record::new(kind, formatter.format_buffer(bytes))
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// RecordKind
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// This enumeration represents log record kind. It is contained inside [`Record`] and helps to determine
/// how to work with log record message content which is different for each log record kind.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum RecordKind {
    /// A manual marker emitted by the user via
    /// [`LoggedStream::log_open`](crate::LoggedStream::log_open) — for example to record the start
    /// of a connection. Never produced automatically.
    Open,
    /// Bytes were read from the wrapped stream.
    Read,
    /// Bytes were written to the wrapped stream.
    Write,
    /// A real IO error occurred while reading or writing (transient `WouldBlock` / `WriteZero`
    /// conditions are skipped).
    Error,
    /// An asynchronous stream was shut down via `poll_shutdown`.
    Shutdown,
    /// The stream wrapper was dropped.
    Drop,
}

impl fmt::Display for RecordKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", char::from(*self))
    }
}

impl From<RecordKind> for char {
    fn from(kind: RecordKind) -> Self {
        match kind {
            RecordKind::Open => '+',
            RecordKind::Read => '<',
            RecordKind::Write => '>',
            RecordKind::Error => '!',
            RecordKind::Shutdown => '-',
            RecordKind::Drop => 'x',
        }
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Tests
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use crate::buffer_formatter::BufferFormatter;
    use crate::buffer_formatter::LowercaseHexadecimalFormatter;
    use crate::record::Event;
    use crate::record::Record;
    use crate::record::RecordKind;
    use std::cell::Cell;

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

    #[test]
    fn test_event_exposes_kind_and_bytes_without_formatting() {
        let formatter = CountingFormatter(Cell::new(0));
        let read = Event::read(&[0x0a, 0xff], &formatter);
        let write = Event::write(&[0x01], &formatter);

        assert_eq!(read.kind(), RecordKind::Read);
        assert_eq!(read.bytes(), Some(&[0x0a, 0xff][..]));
        assert_eq!(write.kind(), RecordKind::Write);
        assert_eq!(write.bytes(), Some(&[0x01][..]));
        assert_eq!(formatter.0.get(), 0);
    }

    #[test]
    fn test_event_formats_once_and_reuses_the_record() {
        let formatter = CountingFormatter(Cell::new(0));
        let event = Event::write(&[0x0a, 0xff], &formatter);

        let first = event.record().clone();
        assert_eq!(first.kind, RecordKind::Write);
        assert_eq!(first.message, "0a:ff");
        assert_eq!(event.record(), &first);

        // The record handed on afterwards is the one already built, timestamp included.
        assert_eq!(event.into_record(), first);
        assert_eq!(formatter.0.get(), 1);
    }

    #[test]
    fn test_event_into_record_formats_when_nothing_asked() {
        let formatter = CountingFormatter(Cell::new(0));
        let event = Event::read(&[0x01, 0x02], &formatter);

        let record = event.into_record();
        assert_eq!(record.kind, RecordKind::Read);
        assert_eq!(record.message, "01:02");
        assert_eq!(formatter.0.get(), 1);
    }

    #[test]
    fn test_event_from_record_has_no_bytes_and_keeps_the_record() {
        let record = Record::new(RecordKind::Error, String::from("Error during read: boom"));
        let event = Event::from(record.clone());

        assert_eq!(event.kind(), RecordKind::Error);
        assert_eq!(event.bytes(), None);
        assert_eq!(event.record(), &record);
        // Timestamp included: the record is handed on as it was built.
        assert_eq!(event.into_record(), record);
    }

    #[test]
    fn test_event_debug_does_not_format() {
        let formatter = CountingFormatter(Cell::new(0));
        let event = Event::read(&[0x01], &formatter);

        let debug = format!("{event:?}");
        assert!(debug.contains("Event"));
        assert!(debug.contains("Read"));
        assert!(debug.contains("bytes: Some([1])"));
        assert!(debug.contains("record: None"));
        assert_eq!(formatter.0.get(), 0);

        // Once formatted, the record is shown.
        event.record();
        assert!(format!("{event:?}").contains("message: \"01\""));

        let deallocated = Event::from(Record::new(RecordKind::Drop, String::from("Deallocated.")));
        let debug = format!("{deallocated:?}");
        assert!(debug.contains("bytes: None"));
        assert!(debug.contains("message: \"Deallocated.\""));
    }
}
