//! Shared test scaffolding: test doubles (IO objects that fail, record what is written or move
//! one byte at a time, and formatters and filters that count or remember what they see) and the
//! `Send` / `Unpin` assertions used across the test submodules.

use crate::BufferFormatter;
use crate::Event;
use crate::EventFilter;
use crate::EventKind;
use crate::LowercaseHexadecimalFormatter;
use crate::Record;
use std::cell::RefCell;
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

/// A synchronous reader whose every `read` fails with the given [`ErrorKind`].
pub(super) struct ErrReader(pub(super) ErrorKind);

impl Read for ErrReader {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(self.0, "boom"))
    }
}

/// A synchronous writer whose every `write` fails with the given [`ErrorKind`].
pub(super) struct ErrWriter(pub(super) ErrorKind);

impl Write for ErrWriter {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(self.0, "boom"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// An asynchronous reader whose every `poll_read` fails with the given [`ErrorKind`].
pub(super) struct ErrAsyncReader(pub(super) ErrorKind);

impl tokio::io::AsyncRead for ErrAsyncReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Poll::Ready(Err(std::io::Error::new(self.0, "boom")))
    }
}

/// An asynchronous writer whose every `poll_write` fails with the given [`ErrorKind`].
pub(super) struct ErrAsyncWriter(pub(super) ErrorKind);

impl tokio::io::AsyncWrite for ErrAsyncWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Poll::Ready(Err(std::io::Error::new(self.0, "boom")))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// A synchronous writer that appends everything written to it into a shared buffer, so a test
/// can assert that bytes actually reach the wrapped stream.
pub(super) struct RecordingWriter(pub(super) Rc<RefCell<Vec<u8>>>);

impl Write for RecordingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Formats like [`LowercaseHexadecimalFormatter`] and counts its `format_buffer` calls. The
/// counter is shared, so a test can read it after the formatter has moved into the stream.
#[derive(Default)]
pub(super) struct CountingFormatter(pub(super) Arc<AtomicUsize>);

impl BufferFormatter for CountingFormatter {
    fn get_separator(&self) -> &str {
        ":"
    }

    fn format_byte(&self, byte: &u8) -> String {
        format!("{byte:02x}")
    }

    fn format_buffer(&self, buffer: &[u8]) -> String {
        self.0.fetch_add(1, Ordering::SeqCst);
        LowercaseHexadecimalFormatter::new_default().format_buffer(buffer)
    }
}

/// Keeps every event, and remembers the formatted record of each.
pub(super) struct SeenRecords(pub(super) Arc<Mutex<Vec<Record>>>);

impl EventFilter for SeenRecords {
    fn check(&self, event: &Event<'_>) -> bool {
        self.0.lock().unwrap().push(event.record().clone());
        true
    }
}

/// Keeps every event, and remembers the kind of each and whether it carried bytes.
pub(super) struct SeenKinds(pub(super) Arc<Mutex<Vec<(EventKind, bool)>>>);

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
pub(super) struct TlsHandshakes(pub(super) Arc<Mutex<Vec<Vec<u8>>>>);

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

/// Asks for the formatted record of every event, keeps the event, and counts its calls.
pub(super) struct CountingFilter(pub(super) Arc<AtomicUsize>);

impl EventFilter for CountingFilter {
    fn check(&self, event: &Event<'_>) -> bool {
        self.0.fetch_add(1, Ordering::SeqCst);
        event.record();
        true
    }
}

/// A synchronous writer that takes at most one byte per `write` call.
pub(super) struct OneByteWriter;

impl Write for OneByteWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len().min(1))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// An asynchronous reader that adds at most one byte of its payload per `poll_read`.
pub(super) struct OneByteAsyncReader(pub(super) Vec<u8>);

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
pub(super) struct OneByteAsyncWriter;

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
// Trait assertions
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

pub(super) fn assert_send<T: Send>() {}

pub(super) fn assert_unpin<T: Unpin>() {}
