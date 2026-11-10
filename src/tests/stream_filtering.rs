//! `LoggedStream` filtering: every event kind reaches the filter but only reads and writes
//! carry bytes, a filter suppresses reads and writes, a read or write is formatted only when a
//! filter asks for its record or it is logged, and a filter that judges raw bytes sees exactly
//! the bytes that moved.

use super::helpers::CountingFilter;
use super::helpers::CountingFormatter;
use super::helpers::ErrAsyncReader;
use super::helpers::RecordingWriter;
use crate::AllFilter;
use crate::ChannelLogger;
use crate::Event;
use crate::EventFilter;
use crate::EventKind;
use crate::EventKindFilter;
use crate::LoggedStream;
use crate::LowercaseHexadecimalFormatter;
use crate::MemoryStorageLogger;
use crate::Record;
use std::cell::RefCell;
use std::io::Cursor;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Test doubles
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Keeps every event, and remembers the formatted record of each.
struct SeenRecords(Arc<Mutex<Vec<Record>>>);

impl EventFilter for SeenRecords {
    fn check(&self, event: &Event<'_>) -> bool {
        self.0.lock().unwrap().push(event.record().clone());
        true
    }
}

/// Keeps every event, and remembers the kind of each and whether it carried bytes.
struct SeenKinds(Arc<Mutex<Vec<(EventKind, bool)>>>);

impl EventFilter for SeenKinds {
    fn check(&self, event: &Event<'_>) -> bool {
        self.0
            .lock()
            .unwrap()
            .push((event.kind(), event.bytes().is_some()));
        true
    }
}

/// Judges reads and writes on their raw bytes, keeping those that start with a TLS handshake
/// byte (0x16), and remembers the bytes it was shown. Of the events without bytes it keeps only
/// drops.
struct TlsHandshakes(Arc<Mutex<Vec<Vec<u8>>>>);

impl EventFilter for TlsHandshakes {
    fn check(&self, event: &Event<'_>) -> bool {
        match event.bytes() {
            Some(bytes) => {
                self.0.lock().unwrap().push(bytes.to_vec());
                bytes.first() == Some(&0x16)
            }
            None => event.kind() == EventKind::Drop,
        }
    }
}

/// A synchronous writer that takes at most one byte per `write` call.
struct OneByteWriter;

impl Write for OneByteWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len().min(1))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// An asynchronous reader that adds at most one byte of its payload per `poll_read`.
struct OneByteAsyncReader(Vec<u8>);

impl tokio::io::AsyncRead for OneByteAsyncReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if !self.0.is_empty() && buf.remaining() > 0 {
            let byte = self.0.remove(0);
            buf.put_slice(&[byte]);
        }
        Poll::Ready(Ok(()))
    }
}

/// An asynchronous writer that takes at most one byte per `poll_write`.
struct OneByteAsyncWriter;

impl tokio::io::AsyncWrite for OneByteAsyncWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Poll::Ready(Ok(buf.len().min(1)))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Event filtering
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn read_suppressed_by_filter() {
    // The filter allows only Write, so the read is not logged.
    let mut stream = LoggedStream::new(
        Cursor::new(vec![0x01, 0x02]),
        LowercaseHexadecimalFormatter::new_default(),
        EventKindFilter::new(&[EventKind::Write]),
        MemoryStorageLogger::new(16),
    );

    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).unwrap();

    assert!(stream.get_log_records().is_empty());
}

#[test]
fn write_suppressed_by_filter() {
    // The filter allows only Read, so the write is not logged.
    let mut stream = LoggedStream::new(
        Cursor::new(Vec::<u8>::new()),
        LowercaseHexadecimalFormatter::new_default(),
        EventKindFilter::new(&[EventKind::Read]),
        MemoryStorageLogger::new(16),
    );

    stream.write_all(&[0x01, 0x02]).unwrap();

    assert!(stream.get_log_records().is_empty());
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Formatted and raw filtering (Event::record / Event::bytes)
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn read_rejected_by_kind_is_not_formatted() {
    let format_calls = Arc::new(AtomicUsize::new(0));
    let mut stream = LoggedStream::new(
        Cursor::new(vec![0x01, 0x02]),
        CountingFormatter(Arc::clone(&format_calls)),
        EventKindFilter::new(&[EventKind::Write]),
        MemoryStorageLogger::new(16),
    );

    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).unwrap();

    assert_eq!(buf, [0x01, 0x02]);
    assert!(stream.get_log_records().is_empty());
    assert_eq!(format_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn write_rejected_by_kind_is_not_formatted() {
    let format_calls = Arc::new(AtomicUsize::new(0));
    let sink = Rc::new(RefCell::new(Vec::new()));
    let mut stream = LoggedStream::new(
        RecordingWriter(Rc::clone(&sink)),
        CountingFormatter(Arc::clone(&format_calls)),
        EventKindFilter::new(&[EventKind::Read]),
        MemoryStorageLogger::new(16),
    );

    stream.write_all(&[0x01, 0x02]).unwrap();

    assert_eq!(*sink.borrow(), vec![0x01, 0x02]);
    assert!(stream.get_log_records().is_empty());
    assert_eq!(format_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn async_read_and_write_rejected_by_kind_are_not_formatted() {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    let format_calls = Arc::new(AtomicUsize::new(0));
    let (client, mut server) = tokio::io::duplex(64);
    server.write_all(&[0xaa, 0xbb]).await.unwrap();

    // Only lifecycle events pass this filter, so reads and writes are rejected by their kind.
    let mut stream = LoggedStream::new(
        client,
        CountingFormatter(Arc::clone(&format_calls)),
        EventKindFilter::new(&[EventKind::Drop, EventKind::Error, EventKind::Shutdown]),
        MemoryStorageLogger::new(16),
    );

    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).await.unwrap();
    stream.write_all(&[0x01, 0x02]).await.unwrap();

    assert_eq!(buf, [0xaa, 0xbb]);
    assert!(stream.get_log_records().is_empty());
    assert_eq!(format_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn formatted_filter_sees_every_read_and_write_formatted() {
    let format_calls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut stream = LoggedStream::new(
        Cursor::new(vec![0x0a, 0xff, 0x00]),
        CountingFormatter(Arc::clone(&format_calls)),
        SeenRecords(Arc::clone(&seen)),
        MemoryStorageLogger::new(16),
    );

    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).unwrap();
    stream.write_all(&[0x01]).unwrap();

    // A filter that asks for the record is shown every read and write formatted, once per IO
    // call, and the logger receives the very records it was shown.
    let seen = seen.lock().unwrap().clone();
    let shown = seen
        .iter()
        .map(|record| (record.kind, record.message.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        shown,
        vec![(EventKind::Read, "0a:ff"), (EventKind::Write, "01")]
    );
    assert_eq!(format_calls.load(Ordering::SeqCst), 2);
    assert_eq!(stream.get_log_records(), seen);
}

#[test]
fn raw_filter_formats_only_the_reads_it_accepts() {
    let format_calls = Arc::new(AtomicUsize::new(0));
    let shown = Arc::new(Mutex::new(Vec::new()));
    let mut stream = LoggedStream::new(
        Cursor::new(vec![0x16, 0x03, 0x17, 0x03]),
        CountingFormatter(Arc::clone(&format_calls)),
        TlsHandshakes(Arc::clone(&shown)),
        MemoryStorageLogger::new(16),
    );

    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).unwrap();
    stream.read_exact(&mut buf).unwrap();

    // The filter saw both reads raw and kept one; only the kept one was formatted.
    assert_eq!(
        *shown.lock().unwrap(),
        vec![vec![0x16, 0x03], vec![0x17, 0x03]]
    );
    let records = stream.get_log_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, EventKind::Read);
    assert_eq!(records[0].message, "16:03");
    assert_eq!(format_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn async_raw_filter_sees_exactly_the_bytes_read_and_written() {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    let shown = Arc::new(Mutex::new(Vec::new()));
    let (client, mut server) = tokio::io::duplex(64);
    server.write_all(&[0x16, 0x01]).await.unwrap();

    let mut stream = LoggedStream::new(
        client,
        LowercaseHexadecimalFormatter::new_default(),
        TlsHandshakes(Arc::clone(&shown)),
        MemoryStorageLogger::new(16),
    );

    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).await.unwrap();
    stream.write_all(&[0x17, 0x02, 0x03]).await.unwrap();

    assert_eq!(
        *shown.lock().unwrap(),
        vec![vec![0x16, 0x01], vec![0x17, 0x02, 0x03]]
    );
    let records = stream.get_log_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].message, "16:01");
}

#[test]
fn raw_filter_judges_events_without_bytes_too() {
    let shown = Arc::new(Mutex::new(Vec::new()));
    let mut stream = LoggedStream::new(
        Cursor::new(vec![0x17, 0x03]),
        LowercaseHexadecimalFormatter::new_default(),
        TlsHandshakes(Arc::clone(&shown)),
        ChannelLogger::new(),
    );
    let receiver = stream.take_receiver_unchecked();

    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).unwrap();
    stream.log_open("rejected, it carries no bytes");
    drop(stream);

    // The read was judged on its bytes and rejected; the open marker and the drop carry no
    // bytes, and of those the filter keeps only drops.
    let records = receiver.try_iter().collect::<Vec<_>>();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, EventKind::Drop);
    assert_eq!(shown.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn only_reads_and_writes_carry_bytes() {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    let seen = Arc::new(Mutex::new(Vec::new()));
    let (client, mut server) = tokio::io::duplex(64);
    server.write_all(&[0x01]).await.unwrap();

    let mut stream = LoggedStream::new(
        client,
        LowercaseHexadecimalFormatter::new_default(),
        SeenKinds(Arc::clone(&seen)),
        MemoryStorageLogger::new(16),
    );
    stream.log_open("open");
    let mut buf = [0u8; 1];
    stream.read_exact(&mut buf).await.unwrap();
    stream.write_all(&[0x02]).await.unwrap();
    stream.shutdown().await.unwrap();
    drop(stream);

    let mut failing = LoggedStream::new(
        ErrAsyncReader(ErrorKind::Other),
        LowercaseHexadecimalFormatter::new_default(),
        SeenKinds(Arc::clone(&seen)),
        MemoryStorageLogger::new(16),
    );
    let _ = failing.read(&mut buf).await;
    drop(failing);

    assert_eq!(
        *seen.lock().unwrap(),
        vec![
            (EventKind::Open, false),
            (EventKind::Read, true),
            (EventKind::Write, true),
            (EventKind::Shutdown, false),
            (EventKind::Drop, false),
            (EventKind::Error, false),
            (EventKind::Drop, false),
        ]
    );
}

#[test]
fn formatting_child_before_kind_filter_formats_every_write() {
    let format_calls = Arc::new(AtomicUsize::new(0));
    let check_calls = Arc::new(AtomicUsize::new(0));
    let mut stream = LoggedStream::new(
        Cursor::new(Vec::<u8>::new()),
        CountingFormatter(Arc::clone(&format_calls)),
        AllFilter::new(vec![
            Box::new(CountingFilter(Arc::clone(&check_calls))),
            Box::new(EventKindFilter::new(&[EventKind::Read])),
        ]),
        MemoryStorageLogger::new(16),
    );

    stream.write_all(&[0x01]).unwrap();
    stream.write_all(&[0x02]).unwrap();

    // The child that asks for the formatted record comes first, so it is shown both writes and
    // they are formatted for it. Asserted before the drop, which the child would see too.
    assert!(stream.get_log_records().is_empty());
    assert_eq!(check_calls.load(Ordering::SeqCst), 2);
    assert_eq!(format_calls.load(Ordering::SeqCst), 2);
}

#[test]
fn kind_filter_before_formatting_child_skips_formatting() {
    let format_calls = Arc::new(AtomicUsize::new(0));
    let check_calls = Arc::new(AtomicUsize::new(0));
    let mut stream = LoggedStream::new(
        Cursor::new(Vec::<u8>::new()),
        CountingFormatter(Arc::clone(&format_calls)),
        AllFilter::new(vec![
            Box::new(EventKindFilter::new(&[EventKind::Read])),
            Box::new(CountingFilter(Arc::clone(&check_calls))),
        ]),
        MemoryStorageLogger::new(16),
    );

    stream.write_all(&[0x01]).unwrap();
    stream.write_all(&[0x02]).unwrap();

    // The kind filter settles both writes first, so the child is skipped and nothing is
    // formatted.
    assert!(stream.get_log_records().is_empty());
    assert_eq!(check_calls.load(Ordering::SeqCst), 0);
    assert_eq!(format_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn raw_filter_sees_only_the_bytes_moved() {
    // A short read: two bytes into an eight-byte buffer.
    let shown = Arc::new(Mutex::new(Vec::new()));
    let mut reader = LoggedStream::new(
        Cursor::new(vec![0x16, 0x01]),
        LowercaseHexadecimalFormatter::new_default(),
        TlsHandshakes(Arc::clone(&shown)),
        MemoryStorageLogger::new(16),
    );
    let mut buf = [0u8; 8];
    assert_eq!(reader.read(&mut buf).unwrap(), 2);
    assert_eq!(*shown.lock().unwrap(), vec![vec![0x16, 0x01]]);
    assert_eq!(reader.get_log_records()[0].message, "16:01");

    // Partial writes: the inner writer takes one byte per call.
    let shown = Arc::new(Mutex::new(Vec::new()));
    let mut writer = LoggedStream::new(
        OneByteWriter,
        LowercaseHexadecimalFormatter::new_default(),
        TlsHandshakes(Arc::clone(&shown)),
        MemoryStorageLogger::new(16),
    );
    writer.write_all(&[0x16, 0x17]).unwrap();
    assert_eq!(*shown.lock().unwrap(), vec![vec![0x16], vec![0x17]]);
    let records = writer.get_log_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].message, "16");
}

#[tokio::test]
async fn async_raw_filter_sees_each_partial_read_and_write() {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    // The inner reader adds one byte per poll, so the second poll starts with one byte already
    // in the buffer; the filter must be shown only the byte that poll added.
    let shown = Arc::new(Mutex::new(Vec::new()));
    let mut reader = LoggedStream::new(
        OneByteAsyncReader(vec![0x16, 0x17]),
        LowercaseHexadecimalFormatter::new_default(),
        TlsHandshakes(Arc::clone(&shown)),
        MemoryStorageLogger::new(16),
    );
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf).await.unwrap();
    assert_eq!(buf, [0x16, 0x17]);
    assert_eq!(*shown.lock().unwrap(), vec![vec![0x16], vec![0x17]]);
    let records = reader.get_log_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].message, "16");

    let shown = Arc::new(Mutex::new(Vec::new()));
    let mut writer = LoggedStream::new(
        OneByteAsyncWriter,
        LowercaseHexadecimalFormatter::new_default(),
        TlsHandshakes(Arc::clone(&shown)),
        MemoryStorageLogger::new(16),
    );
    writer.write_all(&[0x16, 0x17]).await.unwrap();
    assert_eq!(*shown.lock().unwrap(), vec![vec![0x16], vec![0x17]]);
}

#[test]
fn raw_filter_sees_empty_read_at_end_of_stream() {
    let format_calls = Arc::new(AtomicUsize::new(0));
    let shown = Arc::new(Mutex::new(Vec::new()));
    let mut stream = LoggedStream::new(
        Cursor::new(vec![0x16]),
        CountingFormatter(Arc::clone(&format_calls)),
        TlsHandshakes(Arc::clone(&shown)),
        MemoryStorageLogger::new(16),
    );

    let mut data = Vec::new();
    stream.read_to_end(&mut data).unwrap();

    // The end of the stream is a read of zero bytes: the filter is shown an empty slice and
    // rejects it, so only the first read is formatted and logged.
    assert_eq!(data, vec![0x16]);
    let shown = shown.lock().unwrap().clone();
    assert_eq!(shown.first(), Some(&vec![0x16]));
    assert_eq!(shown.last(), Some(&Vec::new()));
    let records = stream.get_log_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].message, "16");
    assert_eq!(format_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn async_read_that_adds_nothing_is_not_passed_to_the_filter() {
    use tokio::io::AsyncReadExt;

    let shown = Arc::new(Mutex::new(Vec::new()));
    let mut stream = LoggedStream::new(
        OneByteAsyncReader(vec![0x16]),
        LowercaseHexadecimalFormatter::new_default(),
        TlsHandshakes(Arc::clone(&shown)),
        MemoryStorageLogger::new(16),
    );

    let mut data = Vec::new();
    stream.read_to_end(&mut data).await.unwrap();

    // The inner reader ends the stream with a poll that adds nothing. Unlike a synchronous read
    // of zero bytes, that poll is not an event, so the filter is shown only the byte read.
    assert_eq!(data, vec![0x16]);
    assert_eq!(*shown.lock().unwrap(), vec![vec![0x16]]);
    let records = stream.get_log_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].message, "16");
}
