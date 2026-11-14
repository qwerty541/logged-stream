//! `LoggedStream` lifecycle events: the manual `Open` marker (`log_open`), the `Drop` event and
//! the `Shutdown` event of an asynchronous shutdown.

use crate::ChannelLogger;
use crate::DefaultFilter;
use crate::EventKind;
use crate::EventKindFilter;
use crate::LoggedStream;
use crate::LowercaseHexadecimalFormatter;
use crate::MemoryStorageLogger;
use std::io::Cursor;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Manual Open marker (log_open)
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn log_open_logs_open_record() {
    let mut stream = LoggedStream::new(
        Cursor::new(Vec::<u8>::new()),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );
    stream.log_open("Established connection with 127.0.0.1:8080");

    let records = stream.get_log_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, EventKind::Open);
    assert_eq!(
        records[0].message,
        "Established connection with 127.0.0.1:8080"
    );
}

#[test]
fn log_open_passes_filter_allowing_open() {
    let mut stream = LoggedStream::new(
        Cursor::new(Vec::<u8>::new()),
        LowercaseHexadecimalFormatter::new_default(),
        EventKindFilter::new(&[EventKind::Open]),
        MemoryStorageLogger::new(16),
    );
    stream.log_open("kept");

    let records = stream.get_log_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, EventKind::Open);
    assert_eq!(records[0].message, "kept");
}

#[test]
fn log_open_suppressed_by_filter_without_open() {
    let mut stream = LoggedStream::new(
        Cursor::new(Vec::<u8>::new()),
        LowercaseHexadecimalFormatter::new_default(),
        EventKindFilter::new(&[EventKind::Read, EventKind::Write]),
        MemoryStorageLogger::new(16),
    );
    stream.log_open("should be filtered out");
    assert!(stream.get_log_records().is_empty());
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Drop & shutdown
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn drop_logs_drop_record() {
    // The ChannelLogger's receiver outlives the stream, so we can observe the Drop record.
    let mut stream = LoggedStream::new(
        Cursor::new(Vec::<u8>::new()),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        ChannelLogger::new(),
    );
    let receiver = stream.take_receiver_unchecked();

    drop(stream);

    let record = receiver
        .recv()
        .expect("dropping the stream should log a record");
    assert_eq!(record.kind, EventKind::Drop);
    assert_eq!(record.message, "Deallocated.");
}

#[tokio::test]
async fn async_shutdown_logs_shutdown_record() {
    use tokio::io::AsyncWriteExt;

    let (client, _server) = tokio::io::duplex(64);
    let mut stream = LoggedStream::new(
        client,
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    stream.shutdown().await.unwrap();

    let records = stream.get_log_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, EventKind::Shutdown);
    assert_eq!(records[0].message, "Writer shutdown request.");
}
