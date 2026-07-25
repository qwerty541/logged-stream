use chrono::DateTime;
use chrono::Utc;
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
