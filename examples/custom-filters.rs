use logged_stream::BufferFormatter;
use logged_stream::Event;
use logged_stream::LoggedStream;
use logged_stream::LowercaseHexadecimalFormatter;
use logged_stream::MemoryStorageLogger;
use logged_stream::RecordFilter;
use logged_stream::RecordKind;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

/// The messages a client sends. The first byte of each is its type: 0x01 is a heartbeat, which
/// only keeps the connection alive, and 0x02 carries data.
const MESSAGES: [[u8; 4]; 6] = [
    [0x01, 0x00, 0x00, 0x00],
    [0x02, 0x00, 0x00, 0x2a],
    [0x01, 0x00, 0x00, 0x00],
    [0x01, 0x00, 0x00, 0x00],
    [0x02, 0x00, 0x00, 0x07],
    [0x01, 0x00, 0x00, 0x00],
];

/// Hides heartbeats by looking at the formatted record: the simplest way to write a filter,
/// because it matches the same text you see in the log. Every read and write is formatted for it.
struct HideHeartbeats;

impl RecordFilter for HideHeartbeats {
    fn check(&self, event: &Event<'_>) -> bool {
        let record = event.record();
        match record.kind {
            // A heartbeat's first byte, 0x01, shows up in the log as "01".
            RecordKind::Read | RecordKind::Write => !record.message.starts_with("01"),
            // Keep errors, shutdowns, drops and opens.
            _ => true,
        }
    }
}

/// Hides heartbeats by looking at the raw bytes, which only reads and writes carry. The reads and
/// writes it rejects are never formatted, which pays off on busy streams.
struct HideHeartbeatsRaw;

impl RecordFilter for HideHeartbeatsRaw {
    fn check(&self, event: &Event<'_>) -> bool {
        match event.bytes() {
            Some(bytes) => bytes.first() != Some(&0x01),
            // Keep errors, shutdowns, drops and opens.
            None => true,
        }
    }
}

/// Formats like `LowercaseHexadecimalFormatter` and counts the buffers it formats, to show what
/// each filter costs.
struct CountingFormatter {
    inner: LowercaseHexadecimalFormatter,
    formatted: Arc<AtomicUsize>,
}

impl BufferFormatter for CountingFormatter {
    fn get_separator(&self) -> &str {
        self.inner.get_separator()
    }

    fn format_byte(&self, byte: &u8) -> String {
        self.inner.format_byte(byte)
    }

    fn format_buffer(&self, buffer: &[u8]) -> String {
        self.formatted.fetch_add(1, Ordering::Relaxed);
        self.inner.format_buffer(buffer)
    }
}

/// This example demonstrates two ways to write a filter of your own: judging the formatted record,
/// or judging the raw bytes. Both filters keep the same records.
fn main() {
    println!("=== Custom Filters Example ===\n");
    println!("A client sends 6 messages; the filters hide the 4 heartbeats (first byte 0x01).\n");

    println!("Example 1: Judge the formatted record");
    println!("-------------------------------------");
    send_messages(HideHeartbeats);

    println!("Example 2: Judge the raw bytes");
    println!("------------------------------");
    send_messages(HideHeartbeatsRaw);

    println!("=== Example Complete ===");
}

/// Sends the messages through a `LoggedStream` that uses `filter`, then prints the records it kept
/// and how many of the writes were formatted.
fn send_messages(filter: impl RecordFilter) {
    let formatted = Arc::new(AtomicUsize::new(0));
    let mut stream = LoggedStream::new(
        // An in-memory buffer stands in for a socket.
        Vec::<u8>::new(),
        CountingFormatter {
            inner: LowercaseHexadecimalFormatter::new_default(),
            formatted: Arc::clone(&formatted),
        },
        filter,
        MemoryStorageLogger::new(16),
    );

    stream.log_open("Connected to the server");
    for message in MESSAGES {
        stream.write_all(&message).unwrap();
    }

    for record in stream.get_log_records() {
        println!("{} {}", record.kind, record.message);
    }
    println!(
        "Writes formatted: {} of {}\n",
        formatted.load(Ordering::Relaxed),
        MESSAGES.len()
    );
}
