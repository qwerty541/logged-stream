use crate::ChannelLogger;
use crate::MemoryStorageLogger;
use crate::RecordFilter;
use crate::buffer_formatter::BufferFormatter;
use crate::logger::Logger;
use crate::record::Record;
use crate::record::RecordKind;
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
/// through them in order: `event -> Formatter -> Filter -> Logger`.
///
/// -   **The inner IO object (`S`).** The stream you are wrapping. [`LoggedStream`] implements the
///     same IO trait `S` does, so it slots in wherever `S` was used.
/// -   **Formatter ([`BufferFormatter`]).** Turns the read and written byte buffers into the display
///     strings you see in the log.
/// -   **Filter ([`RecordFilter`]).** Decides which records are logged. It runs on every record kind,
///     including shutdown and drop.
/// -   **Logger ([`Logger`]).** The sink that consumes accepted records.
///
/// All three of [`BufferFormatter`], [`RecordFilter`] and [`Logger`] are public, `Send + 'static`
/// and object-safe, with blanket implementations for `Box<...>` (and `Arc<T>` where `T: Sync` for
/// [`BufferFormatter`]). You are free to supply your own implementation of any part.
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
/// ## Filters ([`RecordFilter`])
///
/// Decide which records reach the logger.
///
/// | Filter | Behavior |
/// | --- | --- |
/// | [`DefaultFilter`] | Accepts every record. |
/// | [`RecordKindFilter`] | Accepts only the record kinds in an allow-list given at construction. |
/// | [`AllFilter`] | AND — a record passes only if every child filter accepts it (an empty list accepts everything). |
/// | [`AnyFilter`] | OR — a record passes if any child filter accepts it (an empty list rejects everything). |
///
/// ## Loggers ([`Logger`])
///
/// Consume each accepted record.
///
/// | Logger | Destination |
/// | --- | --- |
/// | [`ConsoleLogger`] | Emits records through the `log` facade. |
/// | [`FileLogger`] | Writes records to a file. |
/// | [`MemoryStorageLogger`] | Retains recent records in a bounded in-memory buffer. |
/// | [`ChannelLogger`] | Sends records over an `mpsc` channel for handling elsewhere. |
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
/// [`DefaultFilter`]: crate::DefaultFilter
/// [`RecordKindFilter`]: crate::RecordKindFilter
/// [`AllFilter`]: crate::AllFilter
/// [`AnyFilter`]: crate::AnyFilter
/// [`ConsoleLogger`]: crate::ConsoleLogger
/// [`FileLogger`]: crate::FileLogger
pub struct LoggedStream<
    S: 'static,
    Formatter: 'static,
    Filter: RecordFilter + 'static,
    L: Logger + 'static,
> {
    inner_stream: S,
    formatter: Formatter,
    filter: Filter,
    logger: L,
}

impl<S: 'static, Formatter: 'static, Filter: RecordFilter + 'static, L: Logger + 'static>
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

    /// Emit a custom [`RecordKind::Open`] record carrying `message`.
    ///
    /// [`RecordKind::Open`] is never produced automatically by the read, write, shutdown and drop
    /// machinery — this method is the way to emit one. Use it to annotate the start of a stream,
    /// for example to record the peer of a freshly established connection
    /// (`"Established connection with 127.0.0.1:8080"`) or other per-stream metadata.
    ///
    /// Like every other record, the `Open` record is passed through the filter before it reaches
    /// the logger, so a `RecordKindFilter` that does not allow `Open` will suppress it. The message
    /// is logged verbatim; it is not run through the formatter, which only applies to byte buffers.
    ///
    /// For asynchronous streams, call this before splitting the wrapper with `tokio::io::split`,
    /// since the resulting halves do not expose it.
    pub fn log_open(&mut self, message: impl Into<String>) {
        let record = Record::new(RecordKind::Open, message.into());
        if self.filter.check(&record) {
            self.logger.log(record);
        }
    }
}

impl<S: 'static, Formatter: 'static, Filter: RecordFilter + 'static>
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

impl<S: 'static, Formatter: 'static, Filter: RecordFilter + 'static>
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
    Filter: RecordFilter + fmt::Debug + 'static,
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
    Filter: RecordFilter + 'static,
    L: Logger + 'static,
> io::Read for LoggedStream<S, Formatter, Filter, L>
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let result = self.inner_stream.read(buf);

        match &result {
            Ok(length) => {
                let record = Record::new(
                    RecordKind::Read,
                    self.formatter.format_buffer(&buf[0..*length]),
                );
                if self.filter.check(&record) {
                    self.logger.log(record);
                }
            }
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock) => {}
            Err(e) => self.logger.log(Record::new(
                RecordKind::Error,
                format!("Error during read: {e}"),
            )),
        };

        result
    }
}

impl<
    S: tokio_io::AsyncRead + Unpin + 'static,
    Formatter: BufferFormatter + Unpin + 'static,
    Filter: RecordFilter + Unpin + 'static,
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
            Poll::Ready(Ok(())) => {
                let record = Record::new(
                    RecordKind::Read,
                    mut_self
                        .formatter
                        .format_buffer(&(buf.filled())[length_before_read..length_after_read]),
                );
                if mut_self.filter.check(&record) {
                    mut_self.logger.log(record);
                }
            }
            Poll::Ready(Err(e)) => mut_self.logger.log(Record::new(
                RecordKind::Error,
                format!("Error during async read: {e}"),
            )),
            Poll::Pending => {}
        }

        result
    }
}

impl<
    S: io::Write + 'static,
    Formatter: BufferFormatter + 'static,
    Filter: RecordFilter + 'static,
    L: Logger + 'static,
> io::Write for LoggedStream<S, Formatter, Filter, L>
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let result = self.inner_stream.write(buf);

        match &result {
            Ok(length) => {
                let record = Record::new(
                    RecordKind::Write,
                    self.formatter.format_buffer(&buf[0..*length]),
                );
                if self.filter.check(&record) {
                    self.logger.log(record);
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WriteZero | io::ErrorKind::WouldBlock
                ) => {}
            Err(e) => self.logger.log(Record::new(
                RecordKind::Error,
                format!("Error during write: {e}"),
            )),
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
    Filter: RecordFilter + Unpin + 'static,
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
            Poll::Ready(Ok(length)) => {
                let record = Record::new(
                    RecordKind::Write,
                    mut_self.formatter.format_buffer(&buf[0..*length]),
                );
                if mut_self.filter.check(&record) {
                    mut_self.logger.log(record);
                }
            }
            Poll::Ready(Err(e)) => mut_self.logger.log(Record::new(
                RecordKind::Error,
                format!("Error during async write: {e}"),
            )),
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
        let record = Record::new(
            RecordKind::Shutdown,
            String::from("Writer shutdown request."),
        );
        if mut_self.filter.check(&record) {
            mut_self.logger.log(record);
        }
        result
    }
}

impl<S: 'static, Formatter: 'static, Filter: RecordFilter + 'static, L: Logger + 'static> Drop
    for LoggedStream<S, Formatter, Filter, L>
{
    fn drop(&mut self) {
        let record = Record::new(RecordKind::Drop, String::from("Deallocated."));
        if self.filter.check(&record) {
            self.logger.log(record);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::DefaultFilter;
    use crate::LoggedStream;
    use crate::LowercaseHexadecimalFormatter;
    use crate::MemoryStorageLogger;
    use crate::RecordKind;
    use crate::RecordKindFilter;
    use std::io::Cursor;

    #[test]
    fn test_log_open_emits_open_record() {
        let mut stream = LoggedStream::new(
            Cursor::new(Vec::<u8>::new()),
            LowercaseHexadecimalFormatter::new_default(),
            DefaultFilter,
            MemoryStorageLogger::new(16),
        );
        stream.log_open("Established connection with 127.0.0.1:8080");

        let records = stream.get_log_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, RecordKind::Open);
        assert_eq!(
            records[0].message,
            "Established connection with 127.0.0.1:8080"
        );
    }

    #[test]
    fn test_log_open_suppressed_by_filter_without_open() {
        let mut stream = LoggedStream::new(
            Cursor::new(Vec::<u8>::new()),
            LowercaseHexadecimalFormatter::new_default(),
            RecordKindFilter::new(&[RecordKind::Read, RecordKind::Write]),
            MemoryStorageLogger::new(16),
        );
        stream.log_open("should be filtered out");
        assert!(stream.get_log_records().is_empty());
    }

    #[test]
    fn test_log_open_passes_filter_allowing_open() {
        let mut stream = LoggedStream::new(
            Cursor::new(Vec::<u8>::new()),
            LowercaseHexadecimalFormatter::new_default(),
            RecordKindFilter::new(&[RecordKind::Open]),
            MemoryStorageLogger::new(16),
        );
        stream.log_open("kept");

        let records = stream.get_log_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, RecordKind::Open);
        assert_eq!(records[0].message, "kept");
    }
}
