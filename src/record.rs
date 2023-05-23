use chrono::DateTime;
use chrono::Utc;
use std::convert::From;
use std::fmt;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// Record
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
/// RecordKind
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// This enumeration represents log record kind. It is contained inside [`Record`] and helps to determine
/// how to work with log record message content which is different for each log record kind.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum RecordKind {
    Open,
    Read,
    Write,
    Error,
    Shutdown,
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
