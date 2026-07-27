use crate::RecordKind;
use crate::record::Record;
use std::borrow::Cow;
use std::collections;
use std::fs;
use std::io;
use std::io::Write;
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

/// Logger implementation that writes log records into the provided file.
///
/// This implementation of the [`Logger`] trait writes log records ([`Record`]) into a file, one line per
/// record, in the form `[timestamp] {kind} {message}`.
///
/// Optionally, a prefix can be configured via [`with_prefix`] or [`set_prefix`]. When set, it is written
/// verbatim immediately before the record kind character — that is, after the timestamp — which mirrors
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

    /// Set a prefix that will be written before the record kind character of every line produced by this
    /// logger, and return the modified logger. This is a chainable builder method.
    ///
    /// The prefix is written verbatim between the timestamp and the record kind character — no separator
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

    /// Set or replace the prefix written before the record kind character of every line produced by this
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
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Barrier;
    use std::sync::Once;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::thread;

    // Build a unique temporary file path for a test, so tests running in parallel never share a file.
    // The loggers under test append, so any stale file from an earlier run is removed first.
    fn temp_log_path(tag: &str) -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "logged-stream-{}-{}-{}.log",
            tag,
            std::process::id(),
            unique
        ));
        let _ = fs::remove_file(&path);
        path
    }

    // Split a written line into its bracketed timestamp and everything after it.
    fn split_timestamp(line: &str) -> (&str, &str) {
        let close = line
            .find("] ")
            .expect("line should start with a bracketed timestamp");
        (&line[1..close], &line[close + 2..])
    }

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
        let path = temp_log_path("object-safety");
        let mut file: Box<dyn Logger> = Box::new(FileLogger::open(&path).unwrap());

        let record = Record::new(RecordKind::Open, String::from("test log record"));

        // Assert that trait object methods are dispatchable.
        console.log(record.clone());
        memory.log(record.clone());
        channel.log(record.clone());
        file.log(record);

        drop(file);
        let _ = fs::remove_file(&path);
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

    #[test]
    fn test_file_logger_prefix_default_none() {
        let path = temp_log_path("prefix-default");
        let logger = FileLogger::open(&path).unwrap();

        assert_eq!(logger.prefix(), None);

        drop(logger);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_file_logger_set_and_clear_prefix() {
        let path = temp_log_path("prefix-set");
        let mut logger = FileLogger::open(&path).unwrap();
        assert_eq!(logger.prefix(), None);

        logger.set_prefix(String::from("[server] "));
        assert_eq!(logger.prefix(), Some("[server] "));

        logger.set_prefix("[client] ");
        assert_eq!(logger.prefix(), Some("[client] "));

        logger.clear_prefix();
        assert_eq!(logger.prefix(), None);

        drop(logger);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_file_logger_writes_prefix_after_timestamp() {
        let path = temp_log_path("prefix-placement");
        let mut logger = FileLogger::open(&path).unwrap();

        // Without a prefix, the line keeps the historical `[timestamp] {kind} {message}` shape.
        logger.log(Record::new(RecordKind::Write, String::from("ab:cd")));

        // With a prefix, it is written after the timestamp, immediately before the kind character.
        logger.set_prefix("[conn 5] ");
        logger.log(Record::new(RecordKind::Read, String::from("01:02")));

        // After clearing, subsequent lines are written without any prefix again.
        logger.clear_prefix();
        logger.log(Record::new(
            RecordKind::Shutdown,
            String::from("Writer shutdown request."),
        ));

        drop(logger);

        let content = fs::read_to_string(&path).unwrap();
        let lines = content.lines().collect::<Vec<&str>>();
        assert_eq!(lines.len(), 3);

        let expected = ["> ab:cd", "[conn 5] < 01:02", "- Writer shutdown request."];
        for (line, expected) in lines.iter().zip(expected) {
            assert!(line.starts_with('['), "missing timestamp: {line}");
            let (timestamp, rest) = split_timestamp(line);
            // The part before the prefix must still be a real timestamp, which is what makes the
            // written lines sortable and parseable by log tooling.
            assert!(
                chrono::DateTime::parse_from_rfc3339(timestamp).is_ok(),
                "not a timestamp: {timestamp}"
            );
            assert_eq!(rest, expected);
        }

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_file_logger_concurrent_loggers_do_not_interleave_lines() {
        const THREADS: usize = 8;
        const RECORDS: usize = 150;

        // A payload shaped like the ones this crate actually produces, long enough that rendering it
        // through several small writes would let concurrent loggers splice their lines together.
        let payload = ["ab"; 120].join(":");
        let path = temp_log_path("concurrent");
        let barrier = Arc::new(Barrier::new(THREADS));
        let mut handles = Vec::new();

        for thread_index in 0..THREADS {
            let path = path.clone();
            let payload = payload.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                // One logger per "connection", all appending to the same file.
                let mut logger = FileLogger::open(&path)
                    .unwrap()
                    .with_prefix(format!("[conn {thread_index}] "));
                barrier.wait();
                for _ in 0..RECORDS {
                    logger.log(Record::new(RecordKind::Write, payload.clone()));
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }

        let content = fs::read_to_string(&path).unwrap();
        let lines = content.lines().collect::<Vec<&str>>();
        assert_eq!(
            lines.len(),
            THREADS * RECORDS,
            "records were lost or split across lines"
        );
        for line in lines {
            // Any splicing of two concurrent writes breaks at least one of these invariants.
            assert!(line.starts_with('['), "spliced line: {line}");
            assert_eq!(line.matches("[conn ").count(), 1, "spliced line: {line}");
            assert!(line.ends_with(&payload), "truncated line: {line}");
        }

        let _ = fs::remove_file(&path);
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
