//! `LoggedStream` basics: bytes pass through unchanged, synchronous reads and writes are logged
//! with the configured formatter (a flush is not), the memory and channel loggers' accessors
//! work through the stream, and the trait bounds.

use super::helpers::RecordingWriter;
use super::helpers::assert_send;
use super::helpers::assert_unpin;
use crate::ChannelLogger;
use crate::DecimalFormatter;
use crate::DefaultFilter;
use crate::EventKind;
use crate::LoggedStream;
use crate::LowercaseHexadecimalFormatter;
use crate::MemoryStorageLogger;
use crate::UppercaseHexadecimalFormatter;
use std::cell::RefCell;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::rc::Rc;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Construction & transparency
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn test_read_returns_inner_bytes_unchanged() {
    let data = vec![0x01, 0x02, 0x03, 0x04];
    let mut stream = LoggedStream::new(
        Cursor::new(data.clone()),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf).unwrap();

    assert_eq!(buf.to_vec(), data);
}

#[test]
fn test_write_reaches_inner_stream() {
    let sink = Rc::new(RefCell::new(Vec::new()));
    let mut stream = LoggedStream::new(
        RecordingWriter(Rc::clone(&sink)),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    stream.write_all(&[0xde, 0xad, 0xbe, 0xef]).unwrap();

    assert_eq!(*sink.borrow(), vec![0xde, 0xad, 0xbe, 0xef]);
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Read logging
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn test_read_logs_read_record_with_formatted_content() {
    let mut stream = LoggedStream::new(
        Cursor::new(vec![0x0a, 0xff]),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).unwrap();

    let records = stream.get_log_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, EventKind::Read);
    assert_eq!(records[0].message, "0a:ff");
}

#[test]
fn test_read_uses_configured_formatter() {
    let mut stream = LoggedStream::new(
        Cursor::new(vec![10, 255]),
        DecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).unwrap();

    assert_eq!(stream.get_log_records()[0].message, "10:255");
}

#[test]
fn test_multiple_reads_log_in_order() {
    let mut stream = LoggedStream::new(
        Cursor::new(vec![0x01, 0x02, 0x03, 0x04]),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).unwrap();
    stream.read_exact(&mut buf).unwrap();

    let records = stream.get_log_records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].message, "01:02");
    assert_eq!(records[1].message, "03:04");
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Write logging
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn test_write_logs_write_record_with_formatted_content() {
    let mut stream = LoggedStream::new(
        Cursor::new(Vec::<u8>::new()),
        UppercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    stream.write_all(&[0x0a, 0xff]).unwrap();

    let records = stream.get_log_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, EventKind::Write);
    assert_eq!(records[0].message, "0A:FF");
}

#[test]
fn test_flush_does_not_log() {
    let mut stream = LoggedStream::new(
        Cursor::new(Vec::<u8>::new()),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    stream.flush().unwrap();

    assert!(stream.get_log_records().is_empty());
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Logger accessors
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn test_memory_storage_get_and_clear() {
    let mut stream = LoggedStream::new(
        Cursor::new(vec![0x01, 0x02]),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).unwrap();
    assert_eq!(stream.get_log_records().len(), 1);

    stream.clear_log_records();
    assert!(stream.get_log_records().is_empty());
}

#[test]
fn test_channel_logger_delivers_records() {
    let mut stream = LoggedStream::new(
        Cursor::new(vec![0x01, 0x02]),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        ChannelLogger::new(),
    );
    let receiver = stream.take_receiver_unchecked();

    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).unwrap();

    let record = receiver.recv().unwrap();
    assert_eq!(record.kind, EventKind::Read);
    assert_eq!(record.message, "01:02");
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Trait assertions
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn test_send() {
    assert_send::<
        LoggedStream<
            Cursor<Vec<u8>>,
            LowercaseHexadecimalFormatter,
            DefaultFilter,
            MemoryStorageLogger,
        >,
    >();
}

#[test]
fn test_unpin() {
    assert_unpin::<
        LoggedStream<
            Cursor<Vec<u8>>,
            LowercaseHexadecimalFormatter,
            DefaultFilter,
            MemoryStorageLogger,
        >,
    >();
}

#[test]
fn test_debug() {
    let stream = LoggedStream::new(
        Cursor::new(Vec::<u8>::new()),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(4),
    );

    let debug = format!("{stream:?}");
    assert!(debug.contains("LoggedStream"));
    assert!(debug.contains("formatter"));
}
