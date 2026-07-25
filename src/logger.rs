use crate::RecordKind;
use crate::record::Record;
use std::borrow::Cow;
use std::collections;
use std::io::Write;
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
/// after the log record message has been formatted by an implementation of [`BufferFormatter`] and filtered
/// by an implementation of [`RecordFilter`].
///
/// [`log`]: Logger::log
/// [`LoggedStream`]: crate::LoggedStream
/// [`RecordFilter`]: crate::RecordFilter
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
/// verbatim at the beginning of every log line, before the record kind character. This is useful to
/// disambiguate output when several [`LoggedStream`]s (for example one per connection) log to the same
/// console. No prefix is configured by default.
///
/// [`Error`]: crate::RecordKind::Error
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
    /// The prefix is rendered verbatim immediately before the record kind character — no separator is
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
            RecordKind::Error => log::Level::Error,
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

/// This implementation of [`Logger`] trait writes log records ([`Record`]) into provided file.
pub struct FileLogger {
    file: std::fs::File,
}

impl FileLogger {
    /// Construct a new instance of [`FileLogger`] using provided file.
    pub fn new(file: std::fs::File) -> Self {
        Self { file }
    }
}

impl Logger for FileLogger {
    fn log(&mut self, record: Record) {
        let _ = writeln!(
            self.file,
            "[{}] {} {}",
            record.time.format("%+"),
            record.kind,
            record.message
        );
    }
}

impl Logger for Box<FileLogger> {
    fn log(&mut self, record: Record) {
        (**self).log(record)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Tests
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use crate::logger::ChannelLogger;
    use crate::logger::ConsoleLogger;
    use crate::logger::FileLogger;
    use crate::logger::Logger;
    use crate::logger::MemoryStorageLogger;
    use crate::record::Record;
    use crate::record::RecordKind;
    use std::cell::RefCell;
    use std::sync::Once;

    // A minimal `log::Log` implementation used to capture the exact level and line `ConsoleLogger`
    // emits through the `log` facade. Captured records are stored per-thread, so tests running in
    // parallel never observe each other's output.
    thread_local! {
        static CAPTURED: RefCell<Vec<(log::Level, String)>> = const { RefCell::new(Vec::new()) };
    }

    struct CapturingLogger;

    impl log::Log for CapturingLogger {
        fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
            true
        }

        fn log(&self, record: &log::Record<'_>) {
            CAPTURED.with(|captured| {
                captured
                    .borrow_mut()
                    .push((record.level(), format!("{}", record.args())))
            });
        }

        fn flush(&self) {}
    }

    static CAPTURING_LOGGER: CapturingLogger = CapturingLogger;
    static INIT_CAPTURING_LOGGER: Once = Once::new();

    // Install the capturing logger exactly once for the whole test binary, raise the max level so
    // records are not filtered out, and clear this thread's captured lines to give the calling test
    // a clean slate.
    fn install_capturing_logger() {
        INIT_CAPTURING_LOGGER.call_once(|| {
            // `set_logger` only fails if a logger is already installed; the lib test binary installs
            // none of its own, so this succeeds. Ignore the error defensively.
            let _ = log::set_logger(&CAPTURING_LOGGER);
            log::set_max_level(log::LevelFilter::Trace);
        });
        CAPTURED.with(|captured| captured.borrow_mut().clear());
    }

    fn captured_lines() -> Vec<String> {
        CAPTURED.with(|captured| {
            captured
                .borrow()
                .iter()
                .map(|(_, msg)| msg.clone())
                .collect()
        })
    }

    fn captured_records() -> Vec<(log::Level, String)> {
        CAPTURED.with(|captured| captured.borrow().clone())
    }

    fn assert_unpin<T: Unpin>() {}

    #[test]
    fn test_unpin() {
        assert_unpin::<ConsoleLogger>();
        assert_unpin::<ChannelLogger>();
        assert_unpin::<MemoryStorageLogger>();
        assert_unpin::<FileLogger>();
    }

    #[test]
    fn test_trait_object_safety() {
        // Assert traint object construct.
        let mut console: Box<dyn Logger> = Box::new(ConsoleLogger::new_unchecked("debug"));
        let mut memory: Box<dyn Logger> = Box::new(MemoryStorageLogger::new(100));
        let mut channel: Box<dyn Logger> = Box::new(ChannelLogger::new());

        let record = Record::new(RecordKind::Open, String::from("test log record"));

        // Assert that trait object methods are dispatchable.
        console.log(record.clone());
        memory.log(record.clone());
        channel.log(record);
    }

    fn assert_logger<T: Logger>() {}

    #[test]
    fn test_box() {
        assert_logger::<Box<dyn Logger>>();
        assert_logger::<Box<ConsoleLogger>>();
        assert_logger::<Box<MemoryStorageLogger>>();
        assert_logger::<Box<ChannelLogger>>();
        assert_logger::<Box<FileLogger>>();
    }

    #[test]
    fn test_console_logger_prefix_default_none() {
        assert_eq!(ConsoleLogger::new_unchecked("debug").prefix(), None);
        assert_eq!(ConsoleLogger::new("info").unwrap().prefix(), None);
    }

    #[test]
    fn test_console_logger_with_prefix() {
        // Static string literal.
        let logger = ConsoleLogger::new_unchecked("debug").with_prefix("[conn 5] ");
        assert_eq!(logger.prefix(), Some("[conn 5] "));

        // Owned runtime string (the typical case for a per-connection identifier).
        let id = 42;
        let logger = ConsoleLogger::new_unchecked("debug").with_prefix(format!("[conn {id}] "));
        assert_eq!(logger.prefix(), Some("[conn 42] "));
    }

    #[test]
    fn test_console_logger_set_and_clear_prefix() {
        let mut logger = ConsoleLogger::new_unchecked("debug");
        assert_eq!(logger.prefix(), None);

        logger.set_prefix(String::from("[server] "));
        assert_eq!(logger.prefix(), Some("[server] "));

        logger.set_prefix("[client] ");
        assert_eq!(logger.prefix(), Some("[client] "));

        logger.clear_prefix();
        assert_eq!(logger.prefix(), None);
    }

    #[test]
    fn test_console_logger_logs_prefix_before_kind() {
        install_capturing_logger();

        let mut logger = ConsoleLogger::new_unchecked("debug");

        // Without a prefix, the emitted line matches the historical `"{kind} {message}"` format.
        logger.log(Record::new(RecordKind::Write, String::from("ab:cd")));

        // With a prefix, it is prepended verbatim, before the record kind character.
        logger.set_prefix("[conn 5] ");
        logger.log(Record::new(RecordKind::Read, String::from("01:02")));

        // After clearing, subsequent lines are emitted without any prefix again.
        logger.clear_prefix();
        logger.log(Record::new(
            RecordKind::Shutdown,
            String::from("Writer shutdown request."),
        ));

        assert_eq!(
            captured_lines(),
            vec![
                String::from("> ab:cd"),
                String::from("[conn 5] < 01:02"),
                String::from("- Writer shutdown request."),
            ]
        );
    }

    #[test]
    fn test_console_logger_forces_error_level() {
        install_capturing_logger();

        // The logger is configured at Debug, below Error. Non-error records are emitted at the
        // configured level, but Error records are always forced to `log::Level::Error`.
        let mut logger = ConsoleLogger::new_unchecked("debug");
        logger.log(Record::new(RecordKind::Write, String::from("01:02")));
        logger.log(Record::new(RecordKind::Error, String::from("boom")));

        // A prefix does not change the forced Error level.
        logger.set_prefix("[conn 5] ");
        logger.log(Record::new(RecordKind::Error, String::from("kaboom")));

        assert_eq!(
            captured_records(),
            vec![
                (log::Level::Debug, String::from("> 01:02")),
                (log::Level::Error, String::from("! boom")),
                (log::Level::Error, String::from("[conn 5] ! kaboom")),
            ]
        );
    }

    #[test]
    fn test_console_logger_empty_prefix_matches_no_prefix() {
        install_capturing_logger();

        let mut logger = ConsoleLogger::new_unchecked("debug");
        // No prefix.
        logger.log(Record::new(RecordKind::Write, String::from("01:02")));
        // Empty prefix — documented to produce the same output as no prefix at all.
        logger.set_prefix("");
        logger.log(Record::new(RecordKind::Write, String::from("01:02")));

        let lines = captured_lines();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], lines[1]);
        assert_eq!(lines[0], "> 01:02");
    }

    fn assert_send<T: Send>() {}

    #[test]
    fn test_send() {
        assert_send::<ConsoleLogger>();
        assert_send::<MemoryStorageLogger>();
        assert_send::<ChannelLogger>();
        assert_send::<FileLogger>();

        assert_send::<Box<dyn Logger>>();
        assert_send::<Box<ConsoleLogger>>();
        assert_send::<Box<MemoryStorageLogger>>();
        assert_send::<Box<ChannelLogger>>();
        assert_send::<Box<FileLogger>>();
    }
}
