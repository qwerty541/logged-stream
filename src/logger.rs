use crate::record::Record;
use crate::RecordKind;
use std::collections;
use std::marker::Send;
use std::str::FromStr;
use std::sync::mpsc;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Trait
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

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

#[derive(Debug, Clone)]
pub struct ConsoleLogger {
    level: log::Level,
}

impl ConsoleLogger {
    pub fn new(level: &str) -> Result<Self, log::ParseLevelError> {
        let level = log::Level::from_str(level)?;
        Ok(Self { level })
    }

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

#[derive(Debug, Clone)]
pub struct MemoryStorageLogger {
    storage: collections::VecDeque<Record>,
    max_length: usize,
}

impl MemoryStorageLogger {
    pub fn new(max_length: usize) -> Self {
        Self {
            storage: collections::VecDeque::new(),
            max_length,
        }
    }

    pub fn get_log_records(&self) -> collections::VecDeque<Record> {
        self.storage.clone()
    }

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

#[derive(Debug)]
pub struct ChannelLogger {
    sender: mpsc::Sender<Record>,
    receiver: Option<mpsc::Receiver<Record>>,
}

impl ChannelLogger {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            receiver: Some(receiver),
        }
    }

    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<Record>> {
        self.receiver.take()
    }

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
// Tests
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use crate::logger::ChannelLogger;
    use crate::logger::ConsoleLogger;
    use crate::logger::Logger;
    use crate::logger::MemoryStorageLogger;
    use crate::record::Record;
    use crate::record::RecordKind;
    use std::convert::From;
    use std::marker::Send;
    use std::marker::Unpin;

    fn assert_unpin<T: Unpin>() {}

    #[test]
    fn test_unpin() {
        assert_unpin::<ConsoleLogger>();
        assert_unpin::<ChannelLogger>();
        assert_unpin::<MemoryStorageLogger>();
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
    }

    fn assert_send<T: Send>() {}

    #[test]
    fn test_send() {
        assert_send::<ConsoleLogger>();
        assert_send::<MemoryStorageLogger>();
        assert_send::<ChannelLogger>();

        assert_send::<Box<dyn Logger>>();
        assert_send::<Box<ConsoleLogger>>();
        assert_send::<Box<MemoryStorageLogger>>();
        assert_send::<Box<ChannelLogger>>();
    }
}
