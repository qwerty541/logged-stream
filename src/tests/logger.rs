//! The provided loggers: `ConsoleLogger` (the prefix, the level and the exact line it emits
//! through the `log` facade), `FileLogger` (the prefix after the timestamp, and whole lines from
//! concurrent loggers), and the trait bounds. `MemoryStorageLogger` and `ChannelLogger` are
//! tested through `LoggedStream`, in `stream.rs`.

use super::helpers::assert_send;
use super::helpers::assert_unpin;
use crate::event::EventKind;
use crate::logger::ChannelLogger;
use crate::logger::ConsoleLogger;
use crate::logger::FileLogger;
use crate::logger::Logger;
use crate::logger::MemoryStorageLogger;
use crate::record::Record;
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Barrier;
use std::sync::Once;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// ConsoleLogger
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

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
    logger.log(Record::new(EventKind::Write, String::from("ab:cd")));

    // With a prefix, it is prepended verbatim, before the event kind character.
    logger.set_prefix("[conn 5] ");
    logger.log(Record::new(EventKind::Read, String::from("01:02")));

    // After clearing, subsequent lines are emitted without any prefix again.
    logger.clear_prefix();
    logger.log(Record::new(
        EventKind::Shutdown,
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

    // The logger is configured at Debug, below Error. Non-error records are logged at the
    // configured level, but Error records are always forced to `log::Level::Error`.
    let mut logger = ConsoleLogger::new_unchecked("debug");
    logger.log(Record::new(EventKind::Write, String::from("01:02")));
    logger.log(Record::new(EventKind::Error, String::from("boom")));

    // A prefix does not change the forced Error level.
    logger.set_prefix("[conn 5] ");
    logger.log(Record::new(EventKind::Error, String::from("kaboom")));

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
    logger.log(Record::new(EventKind::Write, String::from("01:02")));
    // Empty prefix — documented to produce the same output as no prefix at all.
    logger.set_prefix("");
    logger.log(Record::new(EventKind::Write, String::from("01:02")));

    let lines = captured_lines();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], lines[1]);
    assert_eq!(lines[0], "> 01:02");
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// FileLogger
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

// Build a unique temporary file path for a test, so tests running in parallel never share a
// file. The loggers under test append, so any stale file from an earlier run is removed first.
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
    logger.log(Record::new(EventKind::Write, String::from("ab:cd")));

    // With a prefix, it is written after the timestamp, immediately before the kind character.
    logger.set_prefix("[conn 5] ");
    logger.log(Record::new(EventKind::Read, String::from("01:02")));

    // After clearing, subsequent lines are written without any prefix again.
    logger.clear_prefix();
    logger.log(Record::new(
        EventKind::Shutdown,
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
                logger.log(Record::new(EventKind::Write, payload.clone()));
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

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Trait assertions
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

fn assert_logger<T: Logger>() {}

#[test]
fn test_unpin() {
    assert_unpin::<ConsoleLogger>();
    assert_unpin::<ChannelLogger>();
    assert_unpin::<MemoryStorageLogger>();
    assert_unpin::<FileLogger>();
}

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

#[test]
fn test_box() {
    assert_logger::<Box<dyn Logger>>();
    assert_logger::<Box<ConsoleLogger>>();
    assert_logger::<Box<MemoryStorageLogger>>();
    assert_logger::<Box<ChannelLogger>>();
    assert_logger::<Box<FileLogger>>();
}

#[test]
fn test_trait_object_safety() {
    // Assert trait object construct.
    let mut console: Box<dyn Logger> = Box::new(ConsoleLogger::new_unchecked("debug"));
    let mut memory: Box<dyn Logger> = Box::new(MemoryStorageLogger::new(100));
    let mut channel: Box<dyn Logger> = Box::new(ChannelLogger::new());
    let path = temp_log_path("object-safety");
    let mut file: Box<dyn Logger> = Box::new(FileLogger::open(&path).unwrap());

    let record = Record::new(EventKind::Open, String::from("test log record"));

    // Assert that trait object methods are dispatchable.
    console.log(record.clone());
    memory.log(record.clone());
    channel.log(record.clone());
    file.log(record);

    drop(file);
    let _ = fs::remove_file(&path);
}
