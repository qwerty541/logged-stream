use crate::record::Record;
use std::collections;
use std::str::FromStr;
use std::sync::mpsc;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// Trait
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

pub trait Logger: 'static {
    fn log(&mut self, record: Record);
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// ConsoleLogger
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
        log::log!(self.level, "{} {}", record.kind, record.message)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// MemoryStorageLogger
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

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// ChannelLogger
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

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// Tests
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use crate::logger::ChannelLogger;
    use crate::logger::ConsoleLogger;
    use crate::logger::MemoryStorageLogger;
    use std::marker::Unpin;

    fn assert_unpin<T: Unpin>() {}

    #[test]
    fn test_unpin() {
        assert_unpin::<ConsoleLogger>();
        assert_unpin::<ChannelLogger>();
        assert_unpin::<MemoryStorageLogger>();
    }
}
