use crate::record::Record;
use crate::RecordKind;
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
/// [`Error`]: crate::RecordKind::Error
#[derive(Debug, Clone)]
pub struct ConsoleLogger {
    level: log::Level,
}

impl ConsoleLogger {
    /// Construct a new instance of [`ConsoleLogger`] using provided log level [`str`]. Returns an [`Err`] in
    /// case if provided log level [`str`] was incorrect.
    pub fn new(level: &str) -> Result<Self, log::ParseLevelError> {
        let level = log::Level::from_str(level)?;
        Ok(Self { level })
    }

    /// Construct a new instance of [`ConsoleLogger`] using provided log level [`str`]. Panics in case if
    /// provided log level [`str`] was incorrect.
    pub fn new_unchecked(level: &str) -> Self {
        Self::new(level).unwrap()
    }
}

impl Logger for ConsoleLogger {
    fn log(&mut self, record: Record) {
        let level = match record.kind {
            RecordKind::Error => log::Level::Error,
            _ => self.level,
        };
        log::log!(level, "{} {}", record.kind, record.message)
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
