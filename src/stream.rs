use crate::ChannelLogger;
use crate::EventFilter;
use crate::MemoryStorageLogger;
use crate::buffer_formatter::BufferFormatter;
use crate::event::Event;
use crate::event::EventKind;
use crate::logger::Logger;
use crate::record::Record;
use std::collections;
use std::fmt;
use std::io;
use std::pin::Pin;
use std::sync::mpsc;
use std::task::Context;
use std::task::Poll;
use tokio::io as tokio_io;

/// Wrapper for an IO object that logs every read, write, error, shutdown and drop that passes
/// through it.
///
/// [`LoggedStream`] wraps an underlying IO object implementing the [`Read`] / [`Write`] traits, or
/// their asynchronous [`tokio`] analogues [`AsyncRead`] / [`AsyncWrite`], and logs all read and
/// write operations, errors, shutdowns and drops. It re-implements the same IO trait it wraps, so
/// it is a drop-in replacement that works transparently in both synchronous and asynchronous code.
///
/// # Architecture
///
/// [`LoggedStream`] is generic over four independent, pluggable parts. Each logged event flows
/// through them as `event -> Formatter -> Filter -> Logger`. The formatter runs lazily: a
/// read or write is formatted only when the filter asks for the formatted record or when the
/// record is logged, so a read or write rejected without it costs almost nothing.
///
/// An *event* ([`Event`]) is something that happened on the stream, such as a read or a shutdown,
/// and it is what the filter judges. A *record* ([`Record`]) is the log entry made from an accepted
/// event, with its formatted message and timestamp, and it is what the logger receives.
///
/// -   **The inner IO object (`S`).** The stream you are wrapping. Any type implementing [`Read`] /
///     [`Write`] (or the [`tokio`] async equivalents) works — a socket, a file, an in-memory buffer,
///     or your own type. [`LoggedStream`] implements the same IO trait `S` does, so it slots in
///     wherever `S` was used.
/// -   **Formatter ([`BufferFormatter`]).** Turns the read and written byte buffers into the display
///     strings you see in the log.
/// -   **Filter ([`EventFilter`]).** Decides which events are logged. It runs on every event kind,
///     including shutdown and drop, and can judge an event by its formatted, human-readable record,
///     the raw bytes of a read or write, or its kind.
/// -   **Logger ([`Logger`]).** The sink that consumes the records of accepted events.
///
/// All three of [`BufferFormatter`], [`EventFilter`] and [`Logger`] are public, `Send + 'static`
/// and object-safe. Each is implemented for its boxed trait object (such as `Box<dyn Logger>`) and
/// for a `Box` of every provided implementation, and [`BufferFormatter`] also for `Arc<T>` where
/// `T: Sync`. You are free to supply your own implementation of any part. A `Box` of your own
/// filter implements [`EventFilter`] too, but a `Box` of your own formatter or logger does not, so
/// box those as trait objects.
///
/// # Provided implementations
///
/// ## Formatters ([`BufferFormatter`])
///
/// Control how byte buffers are rendered. Each formatter stores a separator (default `:`) and
/// exposes parallel constructors: `new`, `new_static`, `new_owned` and `new_default`.
///
/// | Formatter | Renders each byte as |
/// | --- | --- |
/// | [`LowercaseHexadecimalFormatter`] | lowercase hexadecimal — `0a:ff` |
/// | [`UppercaseHexadecimalFormatter`] | uppercase hexadecimal — `0A:FF` |
/// | [`DecimalFormatter`] | decimal — `10:255` |
/// | [`OctalFormatter`] | octal — `012:377` |
/// | [`BinaryFormatter`] | binary — `00001010:11111111` |
///
/// ## Filters ([`EventFilter`])
///
/// Decide which events are logged.
///
/// | Filter | Behavior |
/// | --- | --- |
/// | [`DefaultFilter`] | Accepts every event. |
/// | [`EventKindFilter`] | Accepts only the event kinds in an allow-list given at construction. |
/// | [`AllFilter`] | AND — an event passes only if every child filter accepts it (an empty list accepts everything). |
/// | [`AnyFilter`] | OR — an event passes if any child filter accepts it (an empty list rejects everything). |
///
/// ## Loggers ([`Logger`])
///
/// Consume the record of each accepted event.
///
/// | Logger | Destination |
/// | --- | --- |
/// | [`ConsoleLogger`] | Forwards records to the `log` facade. |
/// | [`FileLogger`] | Writes records to a file, one line per record. |
/// | [`MemoryStorageLogger`] | Retains recent records in a bounded in-memory buffer. |
/// | [`ChannelLogger`] | Sends records over an `mpsc` channel for handling elsewhere. |
///
/// [`ConsoleLogger`] and [`FileLogger`] additionally accept an optional prefix (`with_prefix` /
/// `set_prefix`, none by default), written verbatim before the event kind character of every line.
/// It tells apart several [`LoggedStream`]s — for example one per connection — that share a single
/// console or file:
///
/// ```text
/// [2026-07-20T12:34:56Z] [conn 5] > 01:02:03:04
/// [2026-07-20T12:34:56Z] [conn 7] < 05:06:07:08
/// ```
///
/// To let several [`FileLogger`]s write to one file, construct them with [`FileLogger::open`], which
/// opens the file in append mode. Each record is rendered up front and written with a single
/// `write_all` call, so concurrent loggers never interleave parts of a line. Passing independently
/// opened non-append files (for example from `File::create`) instead gives each logger its own
/// starting offset, and they will silently overwrite each other's records.
///
/// If none of the provided implementations matches your requirements, you can implement
/// [`BufferFormatter`], [`EventFilter`] or [`Logger`] yourself and pass your type to
/// [`LoggedStream`] exactly like a built-in.
///
/// [`Read`]: io::Read
/// [`Write`]: io::Write
/// [`AsyncRead`]: tokio::io::AsyncRead
/// [`AsyncWrite`]: tokio::io::AsyncWrite
/// [`LowercaseHexadecimalFormatter`]: crate::LowercaseHexadecimalFormatter
/// [`UppercaseHexadecimalFormatter`]: crate::UppercaseHexadecimalFormatter
/// [`DecimalFormatter`]: crate::DecimalFormatter
/// [`BinaryFormatter`]: crate::BinaryFormatter
/// [`OctalFormatter`]: crate::OctalFormatter
/// [`Event`]: crate::Event
/// [`DefaultFilter`]: crate::DefaultFilter
/// [`EventKindFilter`]: crate::EventKindFilter
/// [`AllFilter`]: crate::AllFilter
/// [`AnyFilter`]: crate::AnyFilter
/// [`ConsoleLogger`]: crate::ConsoleLogger
/// [`FileLogger`]: crate::FileLogger
/// [`FileLogger::open`]: crate::FileLogger::open
pub struct LoggedStream<
    S: 'static,
    Formatter: 'static,
    Filter: EventFilter + 'static,
    L: Logger + 'static,
> {
    inner_stream: S,
    formatter: Formatter,
    filter: Filter,
    logger: L,
}

impl<S: 'static, Formatter: 'static, Filter: EventFilter + 'static, L: Logger + 'static>
    LoggedStream<S, Formatter, Filter, L>
{
    /// Construct a new instance of [`LoggedStream`] using provided arguments.
    pub fn new(stream: S, formatter: Formatter, filter: Filter, logger: L) -> Self {
        Self {
            inner_stream: stream,
            formatter,
            filter,
            logger,
        }
    }

    /// Emit a custom [`EventKind::Open`] event carrying `message`.
    ///
    /// [`EventKind::Open`] is never produced automatically by the read, write, shutdown and drop
    /// machinery — this method is the way to emit one. Use it to annotate the start of a stream,
    /// for example to note the peer of a freshly established connection
    /// (`"Established connection with 127.0.0.1:8080"`) or other per-stream metadata.
    ///
    /// Like every other event, the `Open` event is passed through the filter before its record
    /// reaches the logger, so an `EventKindFilter` that does not allow `Open` will suppress it. The
    /// message is logged verbatim; it is not run through the formatter, which only applies to byte
    /// buffers.
    ///
    /// For asynchronous streams, call this before splitting the wrapper with `tokio::io::split`,
    /// since the resulting halves do not expose it.
    pub fn log_open(&mut self, message: impl Into<String>) {
        self.emit(EventKind::Open, message);
    }

    /// Route an event without bytes — an error, shutdown, drop or manual `Open` marker — through
    /// the filter, logging its record only if the filter accepts the event.
    ///
    /// Reads and writes go through [`emit_bytes`](Self::emit_bytes) instead. Between them, the two
    /// methods route every event, so the filter is applied consistently to all of them.
    #[inline]
    fn emit(&mut self, kind: EventKind, message: impl Into<String>) {
        let event = Event::message(kind, message);
        if self.filter.check(&event) {
            self.logger.log(event.into_record());
        }
    }

    /// Route the bytes of a read or write through the filter as an [`Event`], logging the
    /// formatted record only if the filter accepts the event.
    ///
    /// The bytes are formatted at most once, and only when needed: when a filter asks for the
    /// formatted record ([`Event::record`]), or when the record of the accepted event is handed to
    /// the logger. A read or write rejected without anyone asking is never formatted.
    #[inline]
    fn emit_bytes(&mut self, kind: EventKind, bytes: &[u8])
    where
        Formatter: BufferFormatter,
    {
        let event = Event::with_bytes(kind, bytes, &self.formatter);
        if self.filter.check(&event) {
            self.logger.log(event.into_record());
        }
    }
}

impl<S: 'static, Formatter: 'static, Filter: EventFilter + 'static>
    LoggedStream<S, Formatter, Filter, MemoryStorageLogger>
{
    #[inline]
    pub fn get_log_records(&self) -> collections::VecDeque<Record> {
        self.logger.get_log_records()
    }

    #[inline]
    pub fn clear_log_records(&mut self) {
        self.logger.clear_log_records()
    }
}

impl<S: 'static, Formatter: 'static, Filter: EventFilter + 'static>
    LoggedStream<S, Formatter, Filter, ChannelLogger>
{
    #[inline]
    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<Record>> {
        self.logger.take_receiver()
    }

    #[inline]
    pub fn take_receiver_unchecked(&mut self) -> mpsc::Receiver<Record> {
        self.logger.take_receiver_unchecked()
    }
}

impl<
    S: fmt::Debug + 'static,
    Formatter: fmt::Debug + 'static,
    Filter: EventFilter + fmt::Debug + 'static,
    L: Logger + fmt::Debug + 'static,
> fmt::Debug for LoggedStream<S, Formatter, Filter, L>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoggedStream")
            .field("inner_stream", &self.inner_stream)
            .field("formatter", &self.formatter)
            .field("filter", &self.filter)
            .field("logger", &self.logger)
            .finish()
    }
}

impl<
    S: io::Read + 'static,
    Formatter: BufferFormatter + 'static,
    Filter: EventFilter + 'static,
    L: Logger + 'static,
> io::Read for LoggedStream<S, Formatter, Filter, L>
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let result = self.inner_stream.read(buf);

        match &result {
            Ok(length) => self.emit_bytes(EventKind::Read, &buf[0..*length]),
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock) => {}
            Err(e) => {
                self.emit(EventKind::Error, format!("Error during read: {e}"));
            }
        };

        result
    }
}

impl<
    S: tokio_io::AsyncRead + Unpin + 'static,
    Formatter: BufferFormatter + Unpin + 'static,
    Filter: EventFilter + Unpin + 'static,
    L: Logger + Unpin + 'static,
> tokio_io::AsyncRead for LoggedStream<S, Formatter, Filter, L>
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio_io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let mut_self = self.get_mut();
        let length_before_read = buf.filled().len();
        let result = Pin::new(&mut mut_self.inner_stream).poll_read(cx, buf);
        let length_after_read = buf.filled().len();
        let diff = length_after_read - length_before_read;

        match &result {
            Poll::Ready(Ok(())) if diff == 0 => {}
            Poll::Ready(Ok(())) => mut_self.emit_bytes(
                EventKind::Read,
                &(buf.filled())[length_before_read..length_after_read],
            ),
            Poll::Ready(Err(e)) => {
                mut_self.emit(EventKind::Error, format!("Error during async read: {e}"));
            }
            Poll::Pending => {}
        }

        result
    }
}

impl<
    S: io::Write + 'static,
    Formatter: BufferFormatter + 'static,
    Filter: EventFilter + 'static,
    L: Logger + 'static,
> io::Write for LoggedStream<S, Formatter, Filter, L>
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let result = self.inner_stream.write(buf);

        match &result {
            Ok(length) => self.emit_bytes(EventKind::Write, &buf[0..*length]),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WriteZero | io::ErrorKind::WouldBlock
                ) => {}
            Err(e) => {
                self.emit(EventKind::Error, format!("Error during write: {e}"));
            }
        };

        result
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner_stream.flush()
    }
}

impl<
    S: tokio_io::AsyncWrite + Unpin + 'static,
    Formatter: BufferFormatter + Unpin + 'static,
    Filter: EventFilter + Unpin + 'static,
    L: Logger + Unpin + 'static,
> tokio_io::AsyncWrite for LoggedStream<S, Formatter, Filter, L>
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let mut_self = self.get_mut();
        let result = Pin::new(&mut mut_self.inner_stream).poll_write(cx, buf);

        match &result {
            Poll::Ready(Ok(length)) => mut_self.emit_bytes(EventKind::Write, &buf[0..*length]),
            Poll::Ready(Err(e)) => {
                mut_self.emit(EventKind::Error, format!("Error during async write: {e}"));
            }
            Poll::Pending => {}
        }

        result
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.get_mut().inner_stream).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let mut_self = self.get_mut();
        let result = Pin::new(&mut mut_self.inner_stream).poll_shutdown(cx);

        mut_self.emit(EventKind::Shutdown, "Writer shutdown request.");

        result
    }
}

impl<S: 'static, Formatter: 'static, Filter: EventFilter + 'static, L: Logger + 'static> Drop
    for LoggedStream<S, Formatter, Filter, L>
{
    fn drop(&mut self) {
        self.emit(EventKind::Drop, "Deallocated.");
    }
}

#[cfg(test)]
mod tests {
    use crate::AllFilter;
    use crate::BufferFormatter;
    use crate::ChannelLogger;
    use crate::DecimalFormatter;
    use crate::DefaultFilter;
    use crate::Event;
    use crate::EventFilter;
    use crate::EventKind;
    use crate::EventKindFilter;
    use crate::LoggedStream;
    use crate::LowercaseHexadecimalFormatter;
    use crate::MemoryStorageLogger;
    use crate::Record;
    use crate::UppercaseHexadecimalFormatter;
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

    //////////////////////////////////////////////////////////////////////////////////////////////////////////
    // Test doubles
    //////////////////////////////////////////////////////////////////////////////////////////////////////////

    /// A synchronous reader whose every `read` fails with the given [`ErrorKind`].
    struct ErrReader(ErrorKind);

    impl Read for ErrReader {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(self.0, "boom"))
        }
    }

    /// A synchronous writer whose every `write` fails with the given [`ErrorKind`].
    struct ErrWriter(ErrorKind);

    impl Write for ErrWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(self.0, "boom"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// An asynchronous reader whose every `poll_read` fails with the given [`ErrorKind`].
    struct ErrAsyncReader(ErrorKind);

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
    struct ErrAsyncWriter(ErrorKind);

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
    struct RecordingWriter(Rc<RefCell<Vec<u8>>>);

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
    struct CountingFormatter(Arc<AtomicUsize>);

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

    /// Asks for the formatted record of every event, keeps the event, and counts its calls.
    struct CountingFilter(Arc<AtomicUsize>);

    impl EventFilter for CountingFilter {
        fn check(&self, event: &Event<'_>) -> bool {
            self.0.fetch_add(1, Ordering::SeqCst);
            event.record();
            true
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

    //////////////////////////////////////////////////////////////////////////////////////////////////////////
    // Construction & transparency
    //////////////////////////////////////////////////////////////////////////////////////////////////////////

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

    //////////////////////////////////////////////////////////////////////////////////////////////////////////
    // Read logging
    //////////////////////////////////////////////////////////////////////////////////////////////////////////

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

    //////////////////////////////////////////////////////////////////////////////////////////////////////////
    // Write logging
    //////////////////////////////////////////////////////////////////////////////////////////////////////////

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

    //////////////////////////////////////////////////////////////////////////////////////////////////////////
    // Event filtering
    //////////////////////////////////////////////////////////////////////////////////////////////////////////

    #[test]
    fn test_read_suppressed_by_filter() {
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
    fn test_write_suppressed_by_filter() {
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

    //////////////////////////////////////////////////////////////////////////////////////////////////////////
    // Formatted and raw filtering (Event::record / Event::bytes)
    //////////////////////////////////////////////////////////////////////////////////////////////////////////

    #[test]
    fn test_read_rejected_by_kind_is_not_formatted() {
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
    fn test_write_rejected_by_kind_is_not_formatted() {
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
    async fn test_async_read_and_write_rejected_by_kind_are_not_formatted() {
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
    fn test_formatted_filter_sees_every_read_and_write_formatted() {
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
    fn test_raw_filter_formats_only_the_reads_it_accepts() {
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
    async fn test_async_raw_filter_sees_exactly_the_bytes_read_and_written() {
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
    fn test_raw_filter_judges_events_without_bytes_too() {
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
    async fn test_only_reads_and_writes_carry_bytes() {
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
    fn test_formatting_child_before_kind_filter_formats_every_write() {
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
    fn test_kind_filter_before_formatting_child_skips_formatting() {
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
    fn test_raw_filter_sees_only_the_bytes_moved() {
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
    async fn test_async_raw_filter_sees_each_partial_read_and_write() {
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
    fn test_raw_filter_sees_empty_read_at_end_of_stream() {
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
    async fn test_async_read_that_adds_nothing_is_not_passed_to_the_filter() {
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

    //////////////////////////////////////////////////////////////////////////////////////////////////////////
    // Error handling (message content, filtering and swallowed error kinds)
    //////////////////////////////////////////////////////////////////////////////////////////////////////////

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

    //////////////////////////////////////////////////////////////////////////////////////////////////////////
    // Manual Open marker (log_open)
    //////////////////////////////////////////////////////////////////////////////////////////////////////////

    #[test]
    fn test_log_open_logs_open_record() {
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
    fn test_log_open_passes_filter_allowing_open() {
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
    fn test_log_open_suppressed_by_filter_without_open() {
        let mut stream = LoggedStream::new(
            Cursor::new(Vec::<u8>::new()),
            LowercaseHexadecimalFormatter::new_default(),
            EventKindFilter::new(&[EventKind::Read, EventKind::Write]),
            MemoryStorageLogger::new(16),
        );
        stream.log_open("should be filtered out");
        assert!(stream.get_log_records().is_empty());
    }

    //////////////////////////////////////////////////////////////////////////////////////////////////////////
    // Lifecycle: drop & shutdown
    //////////////////////////////////////////////////////////////////////////////////////////////////////////

    #[test]
    fn test_drop_logs_drop_record() {
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
    async fn test_async_shutdown_logs_shutdown_record() {
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

    //////////////////////////////////////////////////////////////////////////////////////////////////////////
    // Asynchronous IO
    //////////////////////////////////////////////////////////////////////////////////////////////////////////

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

    //////////////////////////////////////////////////////////////////////////////////////////////////////////
    // Logger accessors
    //////////////////////////////////////////////////////////////////////////////////////////////////////////

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

    //////////////////////////////////////////////////////////////////////////////////////////////////////////
    // Trait assertions
    //////////////////////////////////////////////////////////////////////////////////////////////////////////

    fn assert_send<T: Send>() {}

    fn assert_unpin<T: Unpin>() {}

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
}
