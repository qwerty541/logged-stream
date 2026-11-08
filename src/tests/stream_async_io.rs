//! `LoggedStream` over `AsyncRead` and `AsyncWrite`: asynchronous reads, writes and their
//! errors are logged, and a filter without `Error` suppresses the errors.

use super::helpers::ErrAsyncReader;
use super::helpers::ErrAsyncWriter;
use crate::DefaultFilter;
use crate::EventKind;
use crate::EventKindFilter;
use crate::LoggedStream;
use crate::LowercaseHexadecimalFormatter;
use crate::MemoryStorageLogger;
use std::io::ErrorKind;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Asynchronous IO
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[tokio::test]
async fn test_async_read_logs_read_record() {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    let (client, mut server) = tokio::io::duplex(64);
    server.write_all(&[0xaa, 0xbb]).await.unwrap();

    let mut stream = LoggedStream::new(
        client,
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).await.unwrap();

    assert_eq!(buf, [0xaa, 0xbb]);
    let records = stream.get_log_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, EventKind::Read);
    assert_eq!(records[0].message, "aa:bb");
}

#[tokio::test]
async fn test_async_write_logs_write_record() {
    use tokio::io::AsyncWriteExt;

    let (client, _server) = tokio::io::duplex(64);
    let mut stream = LoggedStream::new(
        client,
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    stream.write_all(&[0x01, 0x02, 0x03, 0x04]).await.unwrap();

    let records = stream.get_log_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, EventKind::Write);
    assert_eq!(records[0].message, "01:02:03:04");
}

#[tokio::test]
async fn test_async_read_error_suppressed_by_filter_without_error() {
    use tokio::io::AsyncReadExt;

    let mut stream = LoggedStream::new(
        ErrAsyncReader(ErrorKind::Other),
        LowercaseHexadecimalFormatter::new_default(),
        EventKindFilter::new(&[EventKind::Read, EventKind::Write]),
        MemoryStorageLogger::new(16),
    );

    let mut buf = [0u8; 4];
    let _ = stream.read(&mut buf).await;

    assert!(stream.get_log_records().is_empty());
}

#[tokio::test]
async fn test_async_write_error_suppressed_by_filter_without_error() {
    use tokio::io::AsyncWriteExt;

    let mut stream = LoggedStream::new(
        ErrAsyncWriter(ErrorKind::Other),
        LowercaseHexadecimalFormatter::new_default(),
        EventKindFilter::new(&[EventKind::Read, EventKind::Write]),
        MemoryStorageLogger::new(16),
    );

    let _ = stream.write(&[0x01, 0x02]).await;

    assert!(stream.get_log_records().is_empty());
}

#[tokio::test]
async fn test_async_read_error_logs_error_record() {
    use tokio::io::AsyncReadExt;

    let mut stream = LoggedStream::new(
        ErrAsyncReader(ErrorKind::Other),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    let mut buf = [0u8; 4];
    let _ = stream.read(&mut buf).await;

    let records = stream.get_log_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, EventKind::Error);
    assert!(records[0].message.starts_with("Error during async read:"));
}

#[tokio::test]
async fn test_async_write_error_logs_error_record() {
    use tokio::io::AsyncWriteExt;

    let mut stream = LoggedStream::new(
        ErrAsyncWriter(ErrorKind::Other),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        MemoryStorageLogger::new(16),
    );

    let _ = stream.write(&[0x01, 0x02]).await;

    let records = stream.get_log_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, EventKind::Error);
    assert!(records[0].message.starts_with("Error during async write:"));
}
