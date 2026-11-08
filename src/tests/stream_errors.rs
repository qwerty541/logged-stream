//! `LoggedStream` errors: the message of an error event, suppressing it with a filter, and the
//! error kinds a synchronous read or write does not log (`WouldBlock`, and `WriteZero` on
//! writes).

use super::helpers::ErrReader;
use super::helpers::ErrWriter;
use crate::DefaultFilter;
use crate::EventKind;
use crate::EventKindFilter;
use crate::LoggedStream;
use crate::LowercaseHexadecimalFormatter;
use crate::MemoryStorageLogger;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Error handling (message content, filtering and swallowed error kinds)
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn test_read_error_logs_error_record() {
    let mut stream = LoggedStream::new(
        ErrReader(ErrorKind::Other),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    let mut buf = [0u8; 4];
    let _ = stream.read(&mut buf);

    let records = stream.get_log_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, EventKind::Error);
    assert!(records[0].message.starts_with("Error during read:"));
}

#[test]
fn test_read_error_suppressed_by_filter_without_error() {
    let mut stream = LoggedStream::new(
        ErrReader(ErrorKind::Other),
        LowercaseHexadecimalFormatter::new_default(),
        EventKindFilter::new(&[EventKind::Read, EventKind::Write]),
        MemoryStorageLogger::new(16),
    );

    let mut buf = [0u8; 4];
    let _ = stream.read(&mut buf);

    assert!(stream.get_log_records().is_empty());
}

#[test]
fn test_read_would_block_is_not_logged() {
    // WouldBlock is a transient non-event and must not produce a record.
    let mut stream = LoggedStream::new(
        ErrReader(ErrorKind::WouldBlock),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    let mut buf = [0u8; 4];
    let _ = stream.read(&mut buf);

    assert!(stream.get_log_records().is_empty());
}

#[test]
fn test_write_error_logs_error_record() {
    let mut stream = LoggedStream::new(
        ErrWriter(ErrorKind::Other),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    let _ = stream.write(&[0x01, 0x02]);

    let records = stream.get_log_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, EventKind::Error);
    assert!(records[0].message.starts_with("Error during write:"));
}

#[test]
fn test_write_error_suppressed_by_filter_without_error() {
    let mut stream = LoggedStream::new(
        ErrWriter(ErrorKind::Other),
        LowercaseHexadecimalFormatter::new_default(),
        EventKindFilter::new(&[EventKind::Read, EventKind::Write]),
        MemoryStorageLogger::new(16),
    );

    let _ = stream.write(&[0x01, 0x02]);

    assert!(stream.get_log_records().is_empty());
}

#[test]
fn test_write_would_block_is_not_logged() {
    let mut stream = LoggedStream::new(
        ErrWriter(ErrorKind::WouldBlock),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    let _ = stream.write(&[0x01, 0x02]);

    assert!(stream.get_log_records().is_empty());
}

#[test]
fn test_write_write_zero_is_not_logged() {
    let mut stream = LoggedStream::new(
        ErrWriter(ErrorKind::WriteZero),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    let _ = stream.write(&[0x01, 0x02]);

    assert!(stream.get_log_records().is_empty());
}
