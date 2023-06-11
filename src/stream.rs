use crate::buffer_formatter::BufferFormatter;
use crate::logger::Logger;
use crate::record::Record;
use crate::record::RecordKind;
use crate::ChannelLogger;
use crate::MemoryStorageLogger;
use crate::RecordFilter;
use std::collections;
use std::convert::From;
use std::fmt;
use std::io;
use std::marker::Unpin;
use std::pin::Pin;
use std::sync::mpsc;
use std::task::Context;
use std::task::Poll;
use tokio::io as tokio_io;

/// This is a structure that can be used as a wrapper for underlying IO object which implements [`Read`]
/// and [`Write`] traits or their asynchronous analogues from [`tokio`] library [`AsyncRead`] and
/// [`AsyncWrite`] to enable logging of all read and write operations, errors and drop.
///
/// [`LoggedStream`] structure constructs from four parts:
///
/// -   Underlying IO object, which must implement [`Write`] and [`Read`] traits or their
/// asynchronous analogues from [`tokio`] library: [`AsyncRead`] and [`AsyncWrite`].
/// -   Buffer formatting part, which must implement [`BufferFormatter`] trait provided by this library.
/// This part of [`LoggedStream`] is responsible for the form you will see the input and
/// output bytes. Currently this library provides the following implementations of [`BufferFormatter`] trait:
/// [`LowercaseHexadecimalFormatter`], [`UppercaseHexadecimalFormatter`], [`DecimalFormatter`],
/// [`BinaryFormatter`] and [`OctalFormatter`]. Also [`BufferFormatter`] is public trait so you are
/// free to construct your own implementation.
/// -   Filtering part, which must implement [`RecordFilter`] trait provide by this library.
/// This part of [`LoggedStream`] is responsible for log records filtering. Currently this library
/// provides the following implementation of [`RecordFilter`] trait: [`DefaultFilter`] which accepts
/// all log records and [`RecordKindFilter`] which accepts logs with kinds specified during construct.
/// Also [`RecordFilter`] is public trait and you are free to construct your own implementation.
/// -   Logging part, which must implement [`Logger`] trait provided by this library. This part
/// of [`LoggedStream`] is responsible for further work with constructed, formatter and filtered
/// log record. For example, it can be outputted to console, written to the file, written to database,
/// written to the memory for further use or sended by the channel. Currently this library provides
/// the following implementations of [`Logger`] trait: [`ConsoleLogger`], [`MemoryStorageLogger`] and [`ChannelLogger`].
/// Also [`Logger`] is public trait and you are free to construct you own implementation.
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
/// [`ConsoleLogger`]: crate::ConsoleLogger
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
}

impl<S: 'static, Formatter: 'static, Filter: RecordFilter + 'static>
    LoggedStream<S, Formatter, Filter, MemoryStorageLogger>
{
    pub fn get_log_records(&self) -> collections::VecDeque<Record> {
        self.logger.get_log_records()
    }

    pub fn clear_log_records(&mut self) {
        self.logger.clear_log_records()
    }
}

impl<S: 'static, Formatter: 'static, Filter: RecordFilter + 'static>
    LoggedStream<S, Formatter, Filter, ChannelLogger>
{
    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<Record>> {
        self.logger.take_receiver()
    }

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
