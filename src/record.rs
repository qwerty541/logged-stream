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
// RawRecord
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// A read or write as seen by [`RecordFilter::check_raw`], before it is formatted.
///
/// It exposes the record kind and the raw bytes exactly as they passed through the stream. The formatted
/// [`Record`] is built the first time [`record`] is called and then reused: by any other filter that asks for
/// it, and as the record handed to the logger. A read or write is therefore formatted at most once. With lazy
/// formatting enabled on the stream ([`LoggedStream::with_lazy_formatting`]), one that is rejected before anything
/// asks for the formatted record is not formatted at all.
///
/// [`record`]: RawRecord::record
/// [`RecordFilter::check_raw`]: crate::RecordFilter::check_raw
/// [`LoggedStream::with_lazy_formatting`]: crate::LoggedStream::with_lazy_formatting
pub struct RawRecord<'a> {
    kind: RecordKind,
    bytes: &'a [u8],
    formatter: &'a dyn BufferFormatter,
    record: OnceCell<Record>,
}

impl<'a> RawRecord<'a> {
    /// Construct a new instance of [`RawRecord`] for `bytes`, which `formatter` renders into the formatted
    /// record on demand.
    ///
    /// [`LoggedStream`] builds these itself; this constructor is meant for testing your own
    /// [`check_raw`] implementation.
    ///
    /// [`LoggedStream`]: crate::LoggedStream
    /// [`check_raw`]: crate::RecordFilter::check_raw
    pub fn new(kind: RecordKind, bytes: &'a [u8], formatter: &'a dyn BufferFormatter) -> Self {
        Self {
            kind,
            bytes,
            formatter,
            record: OnceCell::new(),
        }
    }

    /// Return the record kind: [`RecordKind::Read`] or [`RecordKind::Write`] when built by [`LoggedStream`].
    ///
    /// [`LoggedStream`]: crate::LoggedStream
    #[inline]
    pub fn kind(&self) -> RecordKind {
        self.kind
    }

    /// Return the bytes that were read or written, exactly as they passed through the stream.
    #[inline]
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Return the formatted record. The first call formats the bytes and takes the timestamp; later calls
    /// return the same record.
    ///
    /// The timestamp is therefore taken when the record is first formatted. Without lazy formatting that is
    /// right after the read or write, as before; with it, it is when a filter first asks for the record, or when
    /// the accepted record is handed to the logger.
    pub fn record(&self) -> &Record {
        self.record.get_or_init(|| self.format())
    }

    /// Consume this instance and return its formatted record, formatting it now if nothing asked for it yet.
    pub(crate) fn into_record(mut self) -> Record {
        self.record.take().unwrap_or_else(|| self.format())
    }

    fn format(&self) -> Record {
        Record::new(self.kind, self.formatter.format_buffer(self.bytes))
    }
}

impl fmt::Debug for RawRecord<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawRecord")
            .field("kind", &self.kind)
            .field("bytes", &self.bytes)
            .field("record", &self.record.get())
            .finish_non_exhaustive()
    }
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
    use crate::record::RawRecord;
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
    fn test_raw_record_exposes_kind_and_bytes_without_formatting() {
        let formatter = CountingFormatter(Cell::new(0));
        let raw = RawRecord::new(RecordKind::Read, &[0x0a, 0xff], &formatter);

        assert_eq!(raw.kind(), RecordKind::Read);
        assert_eq!(raw.bytes(), &[0x0a, 0xff]);
        assert_eq!(formatter.0.get(), 0);
    }

    #[test]
    fn test_raw_record_formats_once_and_reuses_the_record() {
        let formatter = CountingFormatter(Cell::new(0));
        let raw = RawRecord::new(RecordKind::Write, &[0x0a, 0xff], &formatter);

        let first = raw.record().clone();
        assert_eq!(first.kind, RecordKind::Write);
        assert_eq!(first.message, "0a:ff");
        assert_eq!(raw.record(), &first);

        // The record handed on afterwards is the one already built, timestamp included.
        assert_eq!(raw.into_record(), first);
        assert_eq!(formatter.0.get(), 1);
    }

    #[test]
    fn test_raw_record_into_record_formats_when_nothing_asked() {
        let formatter = CountingFormatter(Cell::new(0));
        let raw = RawRecord::new(RecordKind::Read, &[0x01, 0x02], &formatter);

        let record = raw.into_record();
        assert_eq!(record.kind, RecordKind::Read);
        assert_eq!(record.message, "01:02");
        assert_eq!(formatter.0.get(), 1);
    }

    #[test]
    fn test_raw_record_debug() {
        let formatter = LowercaseHexadecimalFormatter::new_default();
        let raw = RawRecord::new(RecordKind::Read, &[0x01], &formatter);

        let debug = format!("{raw:?}");
        assert!(debug.contains("RawRecord"));
        assert!(debug.contains("Read"));
        assert!(debug.contains("bytes: [1]"));
        assert!(debug.contains("record: None"));
    }
}
