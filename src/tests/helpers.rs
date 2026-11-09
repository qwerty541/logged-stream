//! Test doubles and assertions used by two or more test files: a failing asynchronous reader, a
//! recording writer, a counting formatter and a counting filter, and the `Send` / `Unpin`
//! assertions.

use crate::BufferFormatter;
use crate::Event;
use crate::EventFilter;
use crate::LowercaseHexadecimalFormatter;
use std::cell::RefCell;
use std::io::ErrorKind;
use std::io::Write;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Test doubles
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

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

/// Asks for the formatted record of every event, keeps the event, and counts its calls.
pub(super) struct CountingFilter(pub(super) Arc<AtomicUsize>);

impl EventFilter for CountingFilter {
    fn check(&self, event: &Event<'_>) -> bool {
        self.0.fetch_add(1, Ordering::SeqCst);
        event.record();
        true
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Trait assertions
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Compiles only if `T` is `Send`.
pub(super) fn assert_send<T: Send>() {}

/// Compiles only if `T` is `Unpin`.
pub(super) fn assert_unpin<T: Unpin>() {}
