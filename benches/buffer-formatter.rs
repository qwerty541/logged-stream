use criterion::criterion_group;
use criterion::criterion_main;
use criterion::Criterion;
use logged_stream::BinaryFormatter;
use logged_stream::BufferFormatter;
use logged_stream::DecimalFormatter;
use logged_stream::LowercaseHexadecimalFormatter;
use logged_stream::OctalFormatter;
use logged_stream::UppercaseHexadecimalFormatter;

const TEST_ARRAY_LENGTH: usize = 1000;

const fn generate_array() -> [u8; TEST_ARRAY_LENGTH] {
    let mut arr = [0; TEST_ARRAY_LENGTH];
    let mut i = 0;
    let mut val: u8 = 0;
    while i < TEST_ARRAY_LENGTH {
        arr[i] = val;
        i += 1;
        val = val.wrapping_add(1);
    }
    arr
}

const FORMATTING_TEST_VALUES: &[u8] = &generate_array();

fn criterion_benchmark(c: &mut Criterion) {
    let lowercase_hexadecimal = LowercaseHexadecimalFormatter::new_default();

    c.bench_function("LowercaseHexadecimalFormatter", move |b| {
        b.iter(|| lowercase_hexadecimal.format_buffer(FORMATTING_TEST_VALUES))
    });

    let uppercase_hexadecimal = UppercaseHexadecimalFormatter::new_default();

    c.bench_function("UppercaseHexadecimalFormatter", move |b| {
        b.iter(|| uppercase_hexadecimal.format_buffer(FORMATTING_TEST_VALUES))
    });

    let decimal = DecimalFormatter::new_default();

    c.bench_function("DecimalFormatter", move |b| {
        b.iter(|| decimal.format_buffer(FORMATTING_TEST_VALUES))
    });

    let octal = OctalFormatter::new_default();

    c.bench_function("OctalFormatter", move |b| {
        b.iter(|| octal.format_buffer(FORMATTING_TEST_VALUES))
    });

    let binary = BinaryFormatter::new_default();

    c.bench_function("BinaryFormatter", move |b| {
        b.iter(|| binary.format_buffer(FORMATTING_TEST_VALUES))
    });
}

criterion_group! {
    name = benches;
    config = if std::env::var("CI").is_ok() {
        // CI mode: faster benchmarks with aggressive time limits
        Criterion::default()
            .noise_threshold(0.05)
            .sample_size(10)
            .warm_up_time(std::time::Duration::from_secs(1))
            .measurement_time(std::time::Duration::from_secs(2))
    } else {
        // Local mode: thorough benchmarks
        Criterion::default()
            .noise_threshold(0.05)
            .sample_size(60)
    };
    targets = criterion_benchmark
}
criterion_main!(benches);
