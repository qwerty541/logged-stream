use chrono::DateTime;
use chrono::Utc;
use std::convert::From;
use std::fmt;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// Record
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub kind: RecordKind,
    pub message: String,
    pub time: DateTime<Utc>,
}

impl Record {
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
