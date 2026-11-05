//! `logged-stream` provides a single wrapper type, [`LoggedStream`], that wraps any underlying IO
//! object and logs every read, write, error, shutdown and drop that passes through it. The wrapper
//! re-implements the same IO trait it wraps — [`Read`] / [`Write`], or the [`tokio`] asynchronous
//! analogues [`AsyncRead`] / [`AsyncWrite`] — so it is a drop-in replacement for the stream it
//! decorates and works transparently in both synchronous and asynchronous code.
//!
//! # Architecture
//!
//! [`LoggedStream`] is generic over four independent, pluggable parts. Each logged event flows
//! through them as `event -> Formatter -> Filter -> Logger`. The formatter runs lazily: a
//! read or write is formatted only when the filter asks for the formatted record or when the
//! record is logged, so a read or write rejected without it costs almost nothing.
//!
//! An *event* ([`Event`]) is something that happened on the stream, such as a read or a shutdown,
//! and it is what the filter judges. A *record* ([`Record`]) is the log entry made from an accepted
//! event, with its formatted message and timestamp, and it is what the logger receives.
//!
//! -   **The inner IO object (`S`).** The stream you are wrapping. Any type implementing [`Read`] /
//!     [`Write`] (or the [`tokio`] async equivalents) works — a socket, a file, an in-memory buffer,
//!     or your own type. [`LoggedStream`] implements the same IO trait `S` does, so it slots in
//!     wherever `S` was used.
//! -   **Formatter ([`BufferFormatter`]).** Turns the read and written byte buffers into the display
//!     strings you see in the log.
//! -   **Filter ([`EventFilter`]).** Decides which events are logged. It runs on every event kind,
//!     including shutdown and drop, and can judge an event by its formatted, human-readable record,
//!     the raw bytes of a read or write, or its kind.
//! -   **Logger ([`Logger`]).** The sink that consumes the records of accepted events.
//!
//! All three of [`BufferFormatter`], [`EventFilter`] and [`Logger`] are public, `Send + 'static`
//! and object-safe. Each is implemented for its boxed trait object (such as `Box<dyn Logger>`) and
//! for a `Box` of every provided implementation, and [`BufferFormatter`] also for `Arc<T>` where
//! `T: Sync`. You are free to supply your own implementation of any part. A `Box` of your own
//! filter implements [`EventFilter`] too, but a `Box` of your own formatter or logger does not, so
//! box those as trait objects.
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
//! ## Filters ([`EventFilter`])
//!
//! Decide which events are logged.
//!
//! | Filter | Behavior |
//! | --- | --- |
//! | [`DefaultFilter`] | Accepts every event. |
//! | [`EventKindFilter`] | Accepts only the event kinds in an allow-list given at construction. |
//! | [`AllFilter`] | AND — an event passes only if every child filter accepts it (an empty list accepts everything). |
//! | [`AnyFilter`] | OR — an event passes if any child filter accepts it (an empty list rejects everything). |
//!
//! ## Loggers ([`Logger`])
//!
//! Consume the record of each accepted event.
//!
//! | Logger | Destination |
//! | --- | --- |
//! | [`ConsoleLogger`] | Forwards records to the `log` facade. |
//! | [`FileLogger`] | Writes records to a file, one line per record. |
//! | [`MemoryStorageLogger`] | Retains recent records in a bounded in-memory buffer. |
//! | [`ChannelLogger`] | Sends records over an `mpsc` channel for handling elsewhere. |
//!
//! [`ConsoleLogger`] and [`FileLogger`] additionally accept an optional prefix (`with_prefix` /
//! `set_prefix`, none by default), written verbatim before the event kind character of every line.
//! It tells apart several [`LoggedStream`]s — for example one per connection — that share a single
//! console or file:
//!
//! ```text
//! [2026-07-20T12:34:56Z] [conn 5] > 01:02:03:04
//! [2026-07-20T12:34:56Z] [conn 7] < 05:06:07:08
//! ```
//!
//! To let several [`FileLogger`]s write to one file, construct them with [`FileLogger::open`], which
//! opens the file in append mode. Each record is rendered up front and written with a single
//! `write_all` call, so concurrent loggers never interleave parts of a line. Passing independently
//! opened non-append files (for example from `File::create`) instead gives each logger its own
//! starting offset, and they will silently overwrite each other's records.
//!
//! If none of the provided implementations matches your requirements, you can implement
//! [`BufferFormatter`], [`EventFilter`] or [`Logger`] yourself and pass your type to
//! [`LoggedStream`] exactly like a built-in.
//!
//! [`Write`]: std::io::Write
//! [`Read`]: std::io::Read
//! [`AsyncRead`]: tokio::io::AsyncRead
//! [`AsyncWrite`]: tokio::io::AsyncWrite

mod buffer_formatter;
mod event;
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
pub use event::Event;
pub use event::EventKind;
pub use filter::AllFilter;
pub use filter::AnyFilter;
pub use filter::DefaultFilter;
pub use filter::EventFilter;
pub use filter::EventKindFilter;
pub use logger::ChannelLogger;
pub use logger::ConsoleLogger;
pub use logger::FileLogger;
pub use logger::Logger;
pub use logger::MemoryStorageLogger;
pub use record::Record;
pub use stream::LoggedStream;
