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
/// -   **The inner IO object (`S`).** The stream you are wrapping. Any type implementing [`Read`] /
///     [`Write`] (or the [`tokio`] async equivalents) works — a socket, a file, an in-memory buffer,
///     or your own type. [`LoggedStream`] implements the same IO trait `S` does, so it slots in
///     wherever `S` was used.
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
/// If none of the provided implementations matches your requirements, you can implement
/// [`BufferFormatter`], [`RecordFilter`] or [`Logger`] yourself and pass your type to
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
        self.emit(Record::new(RecordKind::Open, message.into()));
    }

    /// Route a record through the filter, logging it only if the filter accepts it.
    ///
    /// Every record — reads, writes, errors, shutdowns, drops and manual `Open` markers — is
    /// emitted through this single method, so the filter is applied consistently to all of them.
    #[inline]
    fn emit(&mut self, record: Record) {
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
                self.emit(record);
            }
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock) => {}
            Err(e) => {
                self.emit(Record::new(
                    RecordKind::Error,
                    format!("Error during read: {e}"),
                ));
            }
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
                mut_self.emit(record);
            }
            Poll::Ready(Err(e)) => {
                mut_self.emit(Record::new(
                    RecordKind::Error,
                    format!("Error during async read: {e}"),
                ));
            }
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
                self.emit(record);
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WriteZero | io::ErrorKind::WouldBlock
                ) => {}
            Err(e) => {
                self.emit(Record::new(
                    RecordKind::Error,
                    format!("Error during write: {e}"),
                ));
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
                mut_self.emit(record);
            }
            Poll::Ready(Err(e)) => {
                mut_self.emit(Record::new(
                    RecordKind::Error,
                    format!("Error during async write: {e}"),
                ));
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

        mut_self.emit(Record::new(
            RecordKind::Shutdown,
            String::from("Writer shutdown request."),
        ));

        result
    }
}

impl<S: 'static, Formatter: 'static, Filter: RecordFilter + 'static, L: Logger + 'static> Drop
    for LoggedStream<S, Formatter, Filter, L>
{
    fn drop(&mut self) {
        self.emit(Record::new(RecordKind::Drop, String::from("Deallocated.")));
    }
}

#[cfg(test)]
mod tests {
    use crate::ChannelLogger;
    use crate::DecimalFormatter;
    use crate::DefaultFilter;
    use crate::LoggedStream;
    use crate::LowercaseHexadecimalFormatter;
    use crate::MemoryStorageLogger;
    use crate::RecordKind;
    use crate::RecordKindFilter;
    use crate::UppercaseHexadecimalFormatter;
    use std::cell::RefCell;
    use std::io::Cursor;
    use std::io::ErrorKind;
    use std::io::Read;
    use std::io::Write;
    use std::pin::Pin;
    use std::rc::Rc;
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
        assert_eq!(records[0].kind, RecordKind::Read);
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
        assert_eq!(records[0].kind, RecordKind::Write);
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
    // Record filtering
    //////////////////////////////////////////////////////////////////////////////////////////////////////////

    #[test]
    fn test_read_record_suppressed_by_filter() {
        // The filter allows only Write, so the Read record is dropped.
        let mut stream = LoggedStream::new(
            Cursor::new(vec![0x01, 0x02]),
            LowercaseHexadecimalFormatter::new_default(),
            RecordKindFilter::new(&[RecordKind::Write]),
            MemoryStorageLogger::new(16),
        );

        let mut buf = [0u8; 2];
        stream.read_exact(&mut buf).unwrap();

        assert!(stream.get_log_records().is_empty());
    }

    #[test]
    fn test_write_record_suppressed_by_filter() {
        // The filter allows only Read, so the Write record is dropped.
        let mut stream = LoggedStream::new(
            Cursor::new(Vec::<u8>::new()),
            LowercaseHexadecimalFormatter::new_default(),
            RecordKindFilter::new(&[RecordKind::Read]),
            MemoryStorageLogger::new(16),
        );

        stream.write_all(&[0x01, 0x02]).unwrap();

        assert!(stream.get_log_records().is_empty());
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
        assert_eq!(records[0].kind, RecordKind::Error);
        assert!(records[0].message.starts_with("Error during read:"));
    }

    #[test]
    fn test_read_error_suppressed_by_filter_without_error() {
        let mut stream = LoggedStream::new(
            ErrReader(ErrorKind::Other),
            LowercaseHexadecimalFormatter::new_default(),
            RecordKindFilter::new(&[RecordKind::Read, RecordKind::Write]),
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
        assert_eq!(records[0].kind, RecordKind::Error);
        assert!(records[0].message.starts_with("Error during write:"));
    }

    #[test]
    fn test_write_error_suppressed_by_filter_without_error() {
        let mut stream = LoggedStream::new(
            ErrWriter(ErrorKind::Other),
            LowercaseHexadecimalFormatter::new_default(),
            RecordKindFilter::new(&[RecordKind::Read, RecordKind::Write]),
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
            .expect("dropping the stream should emit a record");
        assert_eq!(record.kind, RecordKind::Drop);
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
        assert_eq!(records[0].kind, RecordKind::Shutdown);
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
        assert_eq!(records[0].kind, RecordKind::Read);
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
        assert_eq!(records[0].kind, RecordKind::Write);
        assert_eq!(records[0].message, "01:02:03:04");
    }

    #[tokio::test]
    async fn test_async_read_error_suppressed_by_filter_without_error() {
        use tokio::io::AsyncReadExt;

        let mut stream = LoggedStream::new(
            ErrAsyncReader(ErrorKind::Other),
            LowercaseHexadecimalFormatter::new_default(),
            RecordKindFilter::new(&[RecordKind::Read, RecordKind::Write]),
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
            RecordKindFilter::new(&[RecordKind::Read, RecordKind::Write]),
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
        assert_eq!(records[0].kind, RecordKind::Error);
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
        assert_eq!(records[0].kind, RecordKind::Error);
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
        assert_eq!(record.kind, RecordKind::Read);
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
