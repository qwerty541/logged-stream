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
