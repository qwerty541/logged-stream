use crate::EventKind;
use crate::record::Record;
use std::borrow::Cow;
use std::collections;
use std::fs;
use std::io::Write;
use std::io::{self};
use std::path::Path;
use std::str::FromStr;
use std::sync::mpsc;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Trait
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Trait for processing log records in [`LoggedStream`].
///
/// This trait allows processing log records ([`Record`]) using the [`log`] method. It should be implemented for
/// structures intended to be used as the logging component within [`LoggedStream`]. The [`log`] method is called
/// by [`LoggedStream`] for further log record processing (e.g., writing to the console, memory, or database)
/// after the event has been accepted by an implementation of [`EventFilter`] and, for a read or write, its bytes
/// have been formatted by an implementation of [`BufferFormatter`].
///
/// [`log`]: Logger::log
/// [`LoggedStream`]: crate::LoggedStream
/// [`EventFilter`]: crate::EventFilter
/// [`BufferFormatter`]: crate::BufferFormatter
pub trait Logger: Send + 'static {
    fn log(&mut self, record: Record);
}

impl Logger for Box<dyn Logger> {
    fn log(&mut self, record: Record) {
        (**self).log(record)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// ConsoleLogger
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Logger implementation that writes log records to the console.
///
/// This implementation of the [`Logger`] trait writes log records ([`Record`]) to the console using the provided
/// [`log::Level`]. Log records with the [`Error`] kind ignore the provided [`log::Level`] and are always written
/// with [`log::Level::Error`].
///
/// Optionally, a prefix can be configured via [`with_prefix`] or [`set_prefix`]. When set, it is printed
/// verbatim at the beginning of every log line, before the event kind character. This is useful to
/// disambiguate output when several [`LoggedStream`]s (for example one per connection) log to the same
/// console. No prefix is configured by default.
///
/// [`Error`]: crate::EventKind::Error
/// [`with_prefix`]: ConsoleLogger::with_prefix
/// [`set_prefix`]: ConsoleLogger::set_prefix
/// [`LoggedStream`]: crate::LoggedStream
#[derive(Debug, Clone)]
pub struct ConsoleLogger {
    level: log::Level,
    prefix: Option<Cow<'static, str>>,
}

impl ConsoleLogger {
    /// Construct a new instance of [`ConsoleLogger`] using the provided log level [`str`]. Returns an
    /// [`Err`] if the provided log level is invalid. The constructed logger has no prefix; use
    /// [`with_prefix`] or [`set_prefix`] to add one.
    ///
    /// [`with_prefix`]: ConsoleLogger::with_prefix
    /// [`set_prefix`]: ConsoleLogger::set_prefix
    pub fn new(level: &str) -> Result<Self, log::ParseLevelError> {
        let level = log::Level::from_str(level)?;
        Ok(Self {
            level,
            prefix: None,
        })
    }

    /// Construct a new instance of [`ConsoleLogger`] using the provided log level [`str`]. Panics if the
    /// provided log level is invalid.
    pub fn new_unchecked(level: &str) -> Self {
        Self::new(level).unwrap()
    }

    /// Set a prefix that will be printed at the beginning of every log line produced by this logger, and
    /// return the modified logger. This is a chainable builder method.
    ///
    /// The prefix is rendered verbatim immediately before the event kind character — no separator is
    /// inserted between them — so include any trailing separator you want yourself (for example a trailing
    /// space or brackets). An empty prefix therefore produces the same output as no prefix at all.
    ///
    /// # Examples
    ///
    /// ```
    /// use logged_stream::ConsoleLogger;
    ///
    /// let logger = ConsoleLogger::new_unchecked("debug").with_prefix("[conn 5] ");
    /// assert_eq!(logger.prefix(), Some("[conn 5] "));
    /// ```
    pub fn with_prefix(mut self, prefix: impl Into<Cow<'static, str>>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Set or replace the prefix printed at the beginning of every log line produced by this logger, in
    /// place. See [`with_prefix`] for details on how the prefix is rendered.
    ///
    /// [`with_prefix`]: ConsoleLogger::with_prefix
    pub fn set_prefix(&mut self, prefix: impl Into<Cow<'static, str>>) {
        self.prefix = Some(prefix.into());
    }

    /// Remove the configured prefix, so log lines are printed without any leading prefix again.
    pub fn clear_prefix(&mut self) {
        self.prefix = None;
    }

    /// Return the currently configured prefix, or [`None`] if no prefix is set.
    #[inline]
    pub fn prefix(&self) -> Option<&str> {
        self.prefix.as_deref()
    }
}

impl Logger for ConsoleLogger {
    fn log(&mut self, record: Record) {
        let level = match record.kind {
            EventKind::Error => log::Level::Error,
            _ => self.level,
        };
        // Format the record straight into the `log::log!` arguments instead of building an
        // intermediate `String`. The prefix-less path is byte-for-byte identical to the historical
        // implementation and allocates nothing beyond what `log` itself does, and both paths keep
        // formatting lazy so nothing is rendered when the level is disabled.
        match self.prefix.as_deref() {
            Some(prefix) => log::log!(level, "{}{} {}", prefix, record.kind, record.message),
            None => log::log!(level, "{} {}", record.kind, record.message),
        }
    }
}

impl Logger for Box<ConsoleLogger> {
    fn log(&mut self, record: Record) {
        (**self).log(record)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// MemoryStorageLogger
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Logger implementation that writes log records to an inner [`VecDeque`] collection.
///
/// This implementation of the [`Logger`] trait writes log records ([`Record`]) into an inner collection
/// ([`collections::VecDeque`]). The length of the inner collection is limited by a number provided during
/// structure construction. You can retrieve accumulated log records from the inner collection using the
/// [`get_log_records`] method and clear the inner collection using the [`clear_log_records`] method.
///
/// [`VecDeque`]: collections::VecDeque
/// [`get_log_records`]: MemoryStorageLogger::get_log_records
/// [`clear_log_records`]: MemoryStorageLogger::clear_log_records
#[derive(Debug, Clone)]
pub struct MemoryStorageLogger {
    storage: collections::VecDeque<Record>,
    max_length: usize,
}

impl MemoryStorageLogger {
    /// Construct a new instance of [`MemoryStorageLogger`] using provided inner collection max length number,
    pub fn new(max_length: usize) -> Self {
        Self {
            storage: collections::VecDeque::new(),
            max_length,
        }
    }

    /// Retrieve log records from inner collection.
    #[inline]
    pub fn get_log_records(&self) -> collections::VecDeque<Record> {
        self.storage.clone()
    }

    /// Clear inner collection of log records.
    #[inline]
    pub fn clear_log_records(&mut self) {
        self.storage.clear()
    }
}

impl Logger for MemoryStorageLogger {
    fn log(&mut self, record: Record) {
        self.storage.push_back(record);
        if self.storage.len() > self.max_length {
            let _ = self.storage.pop_front();
        }
    }
}

impl Logger for Box<MemoryStorageLogger> {
    fn log(&mut self, record: Record) {
        (**self).log(record)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// ChannelLogger
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Logger implementation that sends log records via an asynchronous channel.
///
/// This implementation of the [`Logger`] trait sends log records ([`Record`]) using the sending-half of an underlying
/// asynchronous channel. You can obtain the receiving-half of the channel using the [`take_receiver`] and
/// [`take_receiver_unchecked`] methods.
///
/// [`take_receiver`]: ChannelLogger::take_receiver
/// [`take_receiver_unchecked`]: ChannelLogger::take_receiver_unchecked
#[derive(Debug)]
pub struct ChannelLogger {
    sender: mpsc::Sender<Record>,
    receiver: Option<mpsc::Receiver<Record>>,
}

impl ChannelLogger {
    /// Construct a new instance of [`ChannelLogger`].
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            receiver: Some(receiver),
        }
    }

    /// Take channel receiving-half. Returns [`None`] if it was already taken.
    #[inline]
    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<Record>> {
        self.receiver.take()
    }

    /// Take channel receiving-half. Panics if it was already taken.
    pub fn take_receiver_unchecked(&mut self) -> mpsc::Receiver<Record> {
        self.take_receiver().unwrap()
    }
}

impl Default for ChannelLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl Logger for ChannelLogger {
    fn log(&mut self, record: Record) {
        let _ = self.sender.send(record);
    }
}

impl Logger for Box<ChannelLogger> {
    fn log(&mut self, record: Record) {
        (**self).log(record)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// FileLogger
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Logger implementation that writes log records into the provided file.
///
/// This implementation of the [`Logger`] trait writes log records ([`Record`]) into a file, one line per
/// record, in the form `[timestamp] {kind} {message}`.
///
/// Optionally, a prefix can be configured via [`with_prefix`] or [`set_prefix`]. When set, it is written
/// verbatim immediately before the event kind character — that is, after the timestamp — which mirrors
/// how [`ConsoleLogger`] renders its prefix relative to the timestamp emitted by the logging backend. This
/// is useful to disambiguate output when several [`LoggedStream`]s (for example one per connection) write
/// to the same file. No prefix is configured by default.
///
/// # Sharing one file between several loggers
///
/// Each record is rendered up front and written with a single [`write_all`] call, so concurrent loggers
/// never interleave parts of a line. For that to hold, every logger must write to a file opened in
/// **append** mode — either construct them with [`open`], or share one handle with
/// [`fs::File::try_clone`]. Handing several loggers independently opened non-append files (for example
/// from [`fs::File::create`]) gives each of them its own starting offset, and they will silently
/// overwrite each other's records.
///
/// [`with_prefix`]: FileLogger::with_prefix
/// [`set_prefix`]: FileLogger::set_prefix
/// [`open`]: FileLogger::open
/// [`write_all`]: io::Write::write_all
/// [`ConsoleLogger`]: crate::ConsoleLogger
/// [`LoggedStream`]: crate::LoggedStream
#[derive(Debug)]
pub struct FileLogger {
    file: fs::File,
    prefix: Option<Cow<'static, str>>,
}

impl FileLogger {
    /// Construct a new instance of [`FileLogger`] using the provided file. The constructed logger has no
    /// prefix; use [`with_prefix`] or [`set_prefix`] to add one.
    ///
    /// If the same file is going to be written by several loggers, it must be opened in append mode;
    /// prefer [`open`], which does that for you.
    ///
    /// [`with_prefix`]: FileLogger::with_prefix
    /// [`set_prefix`]: FileLogger::set_prefix
    /// [`open`]: FileLogger::open
    pub fn new(file: fs::File) -> Self {
        Self { file, prefix: None }
    }

    /// Construct a new instance of [`FileLogger`] writing to the file at the provided path, creating the
    /// file if it does not exist and opening it in append mode.
    ///
    /// Append mode is what makes it safe for several loggers — for example one per connection, each with
    /// its own prefix — to write to the same file concurrently without overwriting each other. Returns an
    /// [`Err`] if the file could not be opened.
    ///
    /// # Examples
    ///
    /// ```
    /// use logged_stream::FileLogger;
    ///
    /// let path = std::env::temp_dir().join("logged-stream-open-doctest.log");
    /// let logger = FileLogger::open(&path)?;
    /// # std::fs::remove_file(&path)?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self::new(file))
    }

    /// Set a prefix that will be written before the event kind character of every line produced by this
    /// logger, and return the modified logger. This is a chainable builder method.
    ///
    /// The prefix is written verbatim between the timestamp and the event kind character — no separator
    /// is inserted between the prefix and the kind — so include any trailing separator you want yourself
    /// (for example a trailing space or brackets). An empty prefix therefore produces the same output as
    /// no prefix at all.
    ///
    /// # Examples
    ///
    /// ```
    /// use logged_stream::FileLogger;
    ///
    /// let path = std::env::temp_dir().join("logged-stream-with-prefix-doctest.log");
    /// let logger = FileLogger::open(&path)?.with_prefix("[conn 5] ");
    /// assert_eq!(logger.prefix(), Some("[conn 5] "));
    /// # std::fs::remove_file(&path)?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn with_prefix(mut self, prefix: impl Into<Cow<'static, str>>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Set or replace the prefix written before the event kind character of every line produced by this
    /// logger, in place. See [`with_prefix`] for details on how the prefix is rendered.
    ///
    /// [`with_prefix`]: FileLogger::with_prefix
    pub fn set_prefix(&mut self, prefix: impl Into<Cow<'static, str>>) {
        self.prefix = Some(prefix.into());
    }

    /// Remove the configured prefix, so lines are written without any prefix again.
    pub fn clear_prefix(&mut self) {
        self.prefix = None;
    }

    /// Return the currently configured prefix, or [`None`] if no prefix is set.
    #[inline]
    pub fn prefix(&self) -> Option<&str> {
        self.prefix.as_deref()
    }
}

impl Logger for FileLogger {
    fn log(&mut self, record: Record) {
        // Render the whole line before touching the file, then hand it to a single `write_all`.
        // `std::fs::File` is unbuffered, so writing through `writeln!` would issue one write call per
        // format piece and let concurrent loggers sharing the file splice their lines into each other.
        // This is deliberately the opposite trade-off from `ConsoleLogger`, which formats straight into
        // `log::log!` arguments: there the logging backend does the buffering and locking, here nothing
        // does. The line is rendered into a fresh `String` rather than a buffer reused across calls so
        // that a single large record does not permanently retain its capacity.
        let line = match self.prefix.as_deref() {
            Some(prefix) => format!(
                "[{}] {}{} {}\n",
                record.time.format("%+"),
                prefix,
                record.kind,
                record.message
            ),
            None => format!(
                "[{}] {} {}\n",
                record.time.format("%+"),
                record.kind,
                record.message
            ),
        };
        let _ = self.file.write_all(line.as_bytes());
    }
}

impl Logger for Box<FileLogger> {
    fn log(&mut self, record: Record) {
        (**self).log(record)
    }
}
