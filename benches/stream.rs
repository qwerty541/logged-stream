//! End-to-end cost of one read through `LoggedStream`, depending on how the filter decides.
//!
//! Reads and writes are formatted lazily, so a read that the filter rejects without asking for the
//! formatted record costs about as much as the read itself, while one that has to be formatted pays
//! for the formatter (one allocation per byte with the provided formatters). The `_formatted` cases
//! use filters that implement only `check`, which is also what every rejected read cost before
//! formatting became lazy.
//!
//! Typical times on a laptop (Intel Core i5-8265U, stable Rust 1.98), per read:
//!
//! | Case                       | 64 B   | 8 KiB    |
//! | -------------------------- | ------ | -------- |
//! | `baseline`                 | 2 ns   | 0.09 µs  |
//! | `reject_by_kind`           | 4 ns   | 0.09 µs  |
//! | `reject_by_kind_formatted` | 6 µs   | 0.72 ms  |
//! | `accept`                   | 6 µs   | 0.73 ms  |
//!
//! Absolute times vary with the machine and its load; the gap between formatted and unformatted
//! rejection (three to four orders of magnitude) does not.

use criterion::BenchmarkGroup;
use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::Throughput;
use criterion::criterion_group;
use criterion::criterion_main;
use criterion::measurement::WallTime;
use logged_stream::DefaultFilter;
use logged_stream::LoggedStream;
use logged_stream::LowercaseHexadecimalFormatter;
use logged_stream::MemoryStorageLogger;
use logged_stream::RawRecord;
use logged_stream::Record;
use logged_stream::RecordFilter;
use logged_stream::RecordKind;
use logged_stream::RecordKindFilter;
use std::hint::black_box;
use std::io;
use std::io::Read;

/// Whether `bench_stream` checks that the measured read is logged or rejected.
const LOGGED: bool = true;
const REJECTED: bool = false;

/// Serves the same payload on every read, so a stream can be read in a loop without rewinding.
struct Repeat(Vec<u8>);

impl Read for Repeat {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let length = buf.len().min(self.0.len());
        buf[..length].copy_from_slice(&self.0[..length]);
        Ok(length)
    }
}

/// Rejects reads by kind through `check`, so every read is formatted before it is rejected.
struct RejectReadsFormatted;

impl RecordFilter for RejectReadsFormatted {
    fn check(&self, record: &Record) -> bool {
        record.kind != RecordKind::Read
    }
}

/// Accepts every record through `check`, so every read is formatted for it.
struct AcceptFormatted;

impl RecordFilter for AcceptFormatted {
    fn check(&self, _record: &Record) -> bool {
        true
    }
}

/// Keeps payloads that start with a TLS handshake byte (0x16), judged on the formatted message.
struct HandshakesFormatted;

impl RecordFilter for HandshakesFormatted {
    fn check(&self, record: &Record) -> bool {
        record.message.starts_with("16")
    }
}

/// The same decision as `HandshakesFormatted`, taken on the raw bytes in `check_raw`.
struct HandshakesRaw;

impl RecordFilter for HandshakesRaw {
    fn check(&self, record: &Record) -> bool {
        record.message.starts_with("16")
    }

    fn check_raw(&self, record: &RawRecord<'_>) -> bool {
        record.bytes().first() == Some(&0x16)
    }
}

/// A payload of `size` bytes that does not start with 0x16, so the content filters reject it.
fn payload(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 251) as u8 + 1).collect()
}

/// Benchmark one read of `size` bytes through a `LoggedStream`, after checking once that the read is
/// logged or rejected as the case name promises.
fn bench_stream<F: RecordFilter>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    case: &str,
    size: usize,
    filter: F,
    logged: bool,
) {
    let mut stream = LoggedStream::new(
        Repeat(payload(size)),
        LowercaseHexadecimalFormatter::new_default(),
        filter,
        MemoryStorageLogger::new(1),
    );
    let mut buf = vec![0u8; size];

    let read = stream.read(&mut buf).unwrap();
    assert_eq!(read, size);
    assert_eq!(
        stream.get_log_records().len(),
        usize::from(logged),
        "{case} does not measure what its name says"
    );

    group.bench_function(BenchmarkId::new(case, size), |b| {
        b.iter(|| stream.read(black_box(&mut buf)).unwrap())
    });
}

fn criterion_benchmark(c: &mut Criterion) {
    // Shared CI runners are too noisy for the small size to tell anything, so CI benches only the
    // typical socket buffer.
    let sizes: &[usize] = if std::env::var("CI").is_ok() {
        &[8192]
    } else {
        &[64, 8192]
    };
    let mut group = c.benchmark_group("read");

    for &size in sizes {
        group.throughput(Throughput::Bytes(size as u64));

        // The read itself, without LoggedStream.
        let mut inner = Repeat(payload(size));
        let mut buf = vec![0u8; size];
        group.bench_function(BenchmarkId::new("baseline", size), |b| {
            b.iter(|| inner.read(black_box(&mut buf)).unwrap())
        });

        // Rejected by kind: a filter that implements only `check` needs the formatted record, the
        // provided `RecordKindFilter` decides in `check_raw` without it.
        bench_stream(
            &mut group,
            "reject_by_kind_formatted",
            size,
            RejectReadsFormatted,
            REJECTED,
        );
        bench_stream(
            &mut group,
            "reject_by_kind",
            size,
            RecordKindFilter::new(&[RecordKind::Write]),
            REJECTED,
        );

        // Rejected by content: judged on the formatted message, or on the raw bytes.
        bench_stream(
            &mut group,
            "reject_by_content_formatted",
            size,
            HandshakesFormatted,
            REJECTED,
        );
        bench_stream(
            &mut group,
            "reject_by_content_raw",
            size,
            HandshakesRaw,
            REJECTED,
        );

        // A logged read is formatted once, whichever way the filter decides.
        bench_stream(&mut group, "accept", size, DefaultFilter, LOGGED);
        bench_stream(
            &mut group,
            "accept_formatted",
            size,
            AcceptFormatted,
            LOGGED,
        );
    }

    group.finish();
}

criterion_group! {
    name = stream;
    config = if std::env::var("CI").is_ok() {
        // CI mode: faster benchmarks with aggressive time limits
        Criterion::default()
            .sample_size(10)
            .warm_up_time(std::time::Duration::from_secs(1))
            .measurement_time(std::time::Duration::from_secs(1))
    } else {
        // Local mode: default thorough benchmarks
        Criterion::default()
    };
    targets = criterion_benchmark
}
criterion_main!(stream);
