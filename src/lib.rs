//! This library provides a [`LoggedStream`] structure which can be used as a wrapper for
//! underlying IO object which implements [`Write`] and [`Read`] traits or their
//! asynchronous analogues from [`tokio`] library to enable logging of all read and write
//! operations, errors and drop.
//!
//! [`LoggedStream`] structure constructs from four parts:
//!
//! -   Underlying IO object, which must implement [`Write`] and [`Read`] traits or their
//! asynchronous analogues from [`tokio`] library: [`AsyncRead`] and [`AsyncWrite`].
//! -   Buffer formatting part, which must implement [`BufferFormatter`] trait provided by
//! this library. This part of [`LoggedStream`] is responsible for the form you will see the
//! input and output bytes. Currently this library provides the following implementations of
//! [`BufferFormatter`] trait: [`UppercaseHexadecimalFormatter`], [`LowercaseHexadecimalFormatter`],
//! [`DecimalFormatter`], [`BinaryFormatter`] and [`OctalFormatter`]. Also [`BufferFormatter`] is
//! public trait so you are free to construct your own implementation.
//! -   Filtering part, which must implement [`RecordFilter`] trait provide by this library.
//! This part of [`LoggedStream`] is responsible for log records filtering. Currently this
//! library provides the following implementation of [`RecordFilter`] trait: [`DefaultFilter`] which
//! accepts all log records and [`RecordKindFilter`] which accepts logs with kinds specified during
//! construct. Also [`RecordFilter`] is public trait and you are free to construct your own implementation.
//! -   Logging part, which must implement [`Logger`] trait provided by this library. This part
//! of [`LoggedStream`] is responsible for further work with constructed, formatter and filtered
//! log record. For example, it can be outputted to console, written to the file, written to database,
//! written to the memory for further use or sended by the channel. Currently this library provides
//! the following implementations of [`Logger`] trait: [`ConsoleLogger`], [`MemoryStorageLogger`] and
//! [`ChannelLogger`]. Also [`Logger`] is public trait and you are free to construct you own implementation.
//!
//! [`Write`]: std::io::Write
//! [`Read`]: std::io::Read
//! [`AsyncRead`]: tokio::io::AsyncRead
//! [`AsyncWrite`]: tokio::io::AsyncWrite

mod buffer_formatter;
mod filter;
mod logger;
mod record;
mod stream;

pub use buffer_formatter::BinaryFormatter;
pub use buffer_formatter::BufferFormatter;
pub use buffer_formatter::DecimalFormatter;
pub use buffer_formatter::LowercaseHexadecimalFormatter;
pub use buffer_formatter::OctalFormatter;
pub use buffer_formatter::UppercaseHexadecimalFormatter;
pub use filter::DefaultFilter;
pub use filter::RecordFilter;
pub use filter::RecordKindFilter;
pub use logger::ChannelLogger;
pub use logger::ConsoleLogger;
pub use logger::Logger;
pub use logger::MemoryStorageLogger;
pub use record::Record;
pub use record::RecordKind;
pub use stream::LoggedStream;
