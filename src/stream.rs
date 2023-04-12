use crate::buffer_formatter::BufferFormatter;
use crate::logger::Logger;
use crate::record::Record;
use crate::record::RecordKind;
use crate::ChannelLogger;
use crate::MemoryStorageLogger;
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

pub struct LoggedStream<S: 'static, F: 'static, L: Logger + 'static> {
    inner_stream: S,
    formatter: F,
    logger: L,
}

impl<S: 'static, F: 'static, L: Logger + 'static> LoggedStream<S, F, L> {
    pub fn new(stream: S, formatter: F, logger: L) -> Self {
        Self {
            inner_stream: stream,
            formatter,
            logger,
        }
    }
}

impl<S: 'static, F: 'static> LoggedStream<S, F, MemoryStorageLogger> {
    pub fn get_log_records(&self) -> collections::VecDeque<Record> {
        self.logger.get_log_records()
    }

    pub fn clear_log_records(&mut self) {
        self.logger.clear_log_records()
    }
}

impl<S: 'static, F: 'static> LoggedStream<S, F, ChannelLogger> {
    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<Record>> {
        self.logger.take_receiver()
    }

    pub fn take_receiver_unchecked(&mut self) -> mpsc::Receiver<Record> {
        self.logger.take_receiver_unchecked()
    }
}

impl<S: fmt::Debug + 'static, F: fmt::Debug + 'static, L: Logger + fmt::Debug + 'static> fmt::Debug
    for LoggedStream<S, F, L>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoggedStream")
            .field("inner_stream", &self.inner_stream)
            .field("formatter", &self.formatter)
            .field("logger", &self.logger)
            .finish()
    }
}

impl<S: io::Read + 'static, F: BufferFormatter + 'static, L: Logger + 'static> io::Read
    for LoggedStream<S, F, L>
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let result = self.inner_stream.read(buf);

        match &result {
            Ok(length) => self.logger.log(Record::new(
                RecordKind::Read,
                self.formatter.format_buffer(&buf[0..*length]),
            )),
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
        F: BufferFormatter + Unpin + 'static,
        L: Logger + Unpin + 'static,
    > tokio_io::AsyncRead for LoggedStream<S, F, L>
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
            Poll::Ready(Ok(())) => mut_self.logger.log(Record::new(
                RecordKind::Read,
                mut_self
                    .formatter
                    .format_buffer(&(buf.filled())[length_before_read..length_after_read]),
            )),
            Poll::Ready(Err(e)) => mut_self.logger.log(Record::new(
                RecordKind::Error,
                format!("Error during async read: {e}"),
            )),
            Poll::Pending => {}
        }

        result
    }
}

impl<S: io::Write + 'static, F: BufferFormatter + 'static, L: Logger + 'static> io::Write
    for LoggedStream<S, F, L>
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let result = self.inner_stream.write(buf);

        match &result {
            Ok(length) => self.logger.log(Record::new(
                RecordKind::Write,
                self.formatter.format_buffer(&buf[0..*length]),
            )),
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
        F: BufferFormatter + Unpin + 'static,
        L: Logger + Unpin + 'static,
    > tokio_io::AsyncWrite for LoggedStream<S, F, L>
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let mut_self = self.get_mut();
        let result = Pin::new(&mut mut_self.inner_stream).poll_write(cx, buf);
        match &result {
            Poll::Ready(Ok(length)) => mut_self.logger.log(Record::new(
                RecordKind::Write,
                mut_self.formatter.format_buffer(&buf[0..*length]),
            )),
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
        mut_self.logger.log(Record::new(
            RecordKind::Shutdown,
            String::from("Connection closed by request."),
        ));
        result
    }
}

impl<S: 'static, F: 'static, L: Logger + 'static> Drop for LoggedStream<S, F, L> {
    fn drop(&mut self) {
        self.logger.log(Record::new(
            RecordKind::Drop,
            String::from("Connection socket deallocated."),
        ))
    }
}
