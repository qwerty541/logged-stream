use crate::BufferFormatter;
use crate::Record;
use std::cell::OnceCell;
use std::fmt;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Event
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Something that happened on a [`LoggedStream`]: a read, write, error, shutdown, drop or `Open` marker.
///
/// An event is what a filter judges: [`EventFilter::check`] receives it and decides whether it is logged. Every
/// event has a [kind](Event::kind) and a [record](Event::record), the [`Record`] that the logger receives if the
/// filter accepts the event. A read or write also carries the raw [bytes](Event::bytes) that passed through the
/// stream, and its record is formatted from them the first time it is asked for, then reused: by any other filter
/// that asks for it, and as the record handed to the logger. A read or write is therefore formatted at most once,
/// and one that is rejected before anything asks for its record is not formatted at all. Every other event (an
/// error, shutdown, drop or `Open` marker) carries its record from the start.
///
/// [`LoggedStream`] builds the events itself. To test a filter of your own, build them with [`Event::read`],
/// [`Event::write`] or, for an event without bytes, [`Event::message`]:
///
/// ```
/// use logged_stream::Event;
/// use logged_stream::EventKind;
/// use logged_stream::LowercaseHexadecimalFormatter;
///
/// let formatter = LowercaseHexadecimalFormatter::new_default();
/// let read = Event::read(&[0x16, 0x03, 0x01], &formatter);
/// assert_eq!(read.kind(), EventKind::Read);
/// assert_eq!(read.bytes(), Some(&[0x16, 0x03, 0x01][..]));
/// assert_eq!(read.record().message, "16:03:01");
///
/// let deallocated = Event::message(EventKind::Drop, "Deallocated.");
/// assert_eq!(deallocated.kind(), EventKind::Drop);
/// assert_eq!(deallocated.bytes(), None);
/// assert_eq!(deallocated.record().message, "Deallocated.");
/// ```
///
/// [`LoggedStream`]: crate::LoggedStream
/// [`EventFilter::check`]: crate::EventFilter::check
pub struct Event<'a>(Inner<'a>);

enum Inner<'a> {
    /// A read or write: its bytes, formatted into its record the first time the record is asked for.
    Bytes {
        kind: EventKind,
        bytes: &'a [u8],
        formatter: &'a dyn BufferFormatter,
        record: OnceCell<Record>,
    },
    /// An event without bytes, whose record exists before the filter runs.
    Message(Record),
}

impl<'a> Event<'a> {
    /// Construct a read of `bytes`, which `formatter` renders into its record when the record is asked for.
    #[inline]
    pub fn read(bytes: &'a [u8], formatter: &'a dyn BufferFormatter) -> Self {
        Self::with_bytes(EventKind::Read, bytes, formatter)
    }

    /// Construct a write of `bytes`, which `formatter` renders into its record when the record is asked for.
    #[inline]
    pub fn write(bytes: &'a [u8], formatter: &'a dyn BufferFormatter) -> Self {
        Self::with_bytes(EventKind::Write, bytes, formatter)
    }

    /// Construct an event without bytes that carries `message`, as [`LoggedStream`] does for errors, shutdowns,
    /// drops and `Open` markers. Its record is built, and timestamped, right away.
    ///
    /// The event has no [bytes](Event::bytes), whatever its `kind`, so test a filter that judges bytes with
    /// [`Event::read`] and [`Event::write`] instead.
    ///
    /// [`LoggedStream`]: crate::LoggedStream
    #[inline]
    pub fn message(kind: EventKind, message: impl Into<String>) -> Self {
        Self(Inner::Message(Record::new(kind, message.into())))
    }

    /// Construct a read or write of `bytes`; `kind` is [`EventKind::Read`] or [`EventKind::Write`].
    #[inline]
    pub(crate) fn with_bytes(
        kind: EventKind,
        bytes: &'a [u8],
        formatter: &'a dyn BufferFormatter,
    ) -> Self {
        debug_assert!(matches!(kind, EventKind::Read | EventKind::Write));
        Self(Inner::Bytes {
            kind,
            bytes,
            formatter,
            record: OnceCell::new(),
        })
    }

    /// Return the kind of the event.
    #[inline]
    pub fn kind(&self) -> EventKind {
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
    /// passed to the filter at all. An event built with [`Event::message`] has none, whatever its kind.
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

fn format(kind: EventKind, bytes: &[u8], formatter: &dyn BufferFormatter) -> Record {
    Record::new(kind, formatter.format_buffer(bytes))
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// EventKind
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// This enumeration represents the kind of an [`Event`]. The [`Record`] made from an event carries the same kind,
/// which helps to determine how to work with its message content, different for each kind.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// A manual marker emitted by the user via
    /// [`LoggedStream::log_open`](crate::LoggedStream::log_open) — for example to mark the start
    /// of a connection. Never produced automatically.
    Open,
    /// Bytes were read from the wrapped stream.
    Read,
    /// Bytes were written to the wrapped stream.
    Write,
    /// A read or write returned an IO error. `WouldBlock` on a synchronous read or write and
    /// `WriteZero` on a synchronous write produce no event; asynchronous reads and writes have no
    /// such exceptions.
    Error,
    /// An asynchronous stream was shut down via `poll_shutdown`.
    Shutdown,
    /// The stream wrapper was dropped.
    Drop,
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", char::from(*self))
    }
}

impl From<EventKind> for char {
    fn from(kind: EventKind) -> Self {
        match kind {
            EventKind::Open => '+',
            EventKind::Read => '<',
            EventKind::Write => '>',
            EventKind::Error => '!',
            EventKind::Shutdown => '-',
            EventKind::Drop => 'x',
        }
    }
}
