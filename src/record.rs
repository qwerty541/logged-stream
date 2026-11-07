use crate::EventKind;
use chrono::DateTime;
use chrono::Utc;

/// This structure represents a log record, the log entry made from an accepted [`Event`], and contains message
/// string, creation timestamp ([`DateTime`]<[`Utc`]>) and the kind of that event ([`EventKind`]).
///
/// [`Event`]: crate::Event
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Record {
    pub kind: EventKind,
    pub message: String,
    pub time: DateTime<Utc>,
}

impl Record {
    /// Construct a new instance of [`Record`] using provided message and kind.
    pub fn new(kind: EventKind, message: String) -> Self {
        Self {
            kind,
            message,
            time: Utc::now(),
        }
    }
}
