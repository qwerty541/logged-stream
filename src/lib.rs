//! `logged-stream` provides a single wrapper type, [`LoggedStream`], that wraps any underlying IO
//! object and logs every read, write, error, shutdown and drop that passes through it. The wrapper
//! re-implements the same IO trait it wraps — [`Read`] / [`Write`], or the [`tokio`] asynchronous
//! analogues [`AsyncRead`] / [`AsyncWrite`] — so it is a drop-in replacement for the stream it
//! decorates and works transparently in both synchronous and asynchronous code.
//!
//! # Architecture
//!
//! [`LoggedStream`] is generic over four independent, pluggable parts. Each logged event flows
//! through them in order: `event -> Formatter -> Filter -> Logger`.
//!
//! -   **The inner IO object (`S`).** The stream you are wrapping. [`LoggedStream`] implements the
//!     same IO trait `S` does, so it slots in wherever `S` was used.
//! -   **Formatter ([`BufferFormatter`]).** Turns the read and written byte buffers into the display
//!     strings you see in the log.
//! -   **Filter ([`RecordFilter`]).** Decides which records are logged. It runs on every record kind,
//!     including shutdown and drop.
//! -   **Logger ([`Logger`]).** The sink that consumes accepted records.
//!
//! All three of [`BufferFormatter`], [`RecordFilter`] and [`Logger`] are public, `Send + 'static`
//! and object-safe, with blanket implementations for `Box<...>` (and `Arc<T>` where `T: Sync` for
//! [`BufferFormatter`]). You are free to supply your own implementation of any part.
//!
//! # Provided implementations
//!
//! ## Formatters ([`BufferFormatter`])
//!
//! Control how byte buffers are rendered. Each formatter stores a separator (default `:`) and
//! exposes parallel constructors: `new`, `new_static`, `new_owned` and `new_default`.
//!
//! | Formatter | Renders each byte as |
//! | --- | --- |
//! | [`LowercaseHexadecimalFormatter`] | lowercase hexadecimal — `0a:ff` |
//! | [`UppercaseHexadecimalFormatter`] | uppercase hexadecimal — `0A:FF` |
//! | [`DecimalFormatter`] | decimal — `10:255` |
//! | [`OctalFormatter`] | octal — `012:377` |
//! | [`BinaryFormatter`] | binary — `00001010:11111111` |
//!
//! ## Filters ([`RecordFilter`])
//!
//! Decide which records reach the logger.
//!
//! | Filter | Behavior |
//! | --- | --- |
//! | [`DefaultFilter`] | Accepts every record. |
//! | [`RecordKindFilter`] | Accepts only the record kinds in an allow-list given at construction. |
//! | [`AllFilter`] | AND — a record passes only if every child filter accepts it (an empty list accepts everything). |
//! | [`AnyFilter`] | OR — a record passes if any child filter accepts it (an empty list rejects everything). |
//!
//! ## Loggers ([`Logger`])
//!
//! Consume each accepted record.
//!
//! | Logger | Destination |
//! | --- | --- |
//! | [`ConsoleLogger`] | Emits records through the `log` facade. |
//! | [`FileLogger`] | Writes records to a file. |
//! | [`MemoryStorageLogger`] | Retains recent records in a bounded in-memory buffer. |
//! | [`ChannelLogger`] | Sends records over an `mpsc` channel for handling elsewhere. |
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
pub use filter::AllFilter;
pub use filter::AnyFilter;
pub use filter::DefaultFilter;
pub use filter::RecordFilter;
pub use filter::RecordKindFilter;
pub use logger::ChannelLogger;
pub use logger::ConsoleLogger;
pub use logger::FileLogger;
pub use logger::Logger;
pub use logger::MemoryStorageLogger;
pub use record::Record;
pub use record::RecordKind;
pub use stream::LoggedStream;
