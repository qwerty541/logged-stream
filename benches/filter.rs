use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use logged_stream::AllFilter;
use logged_stream::AnyFilter;
use logged_stream::Event;
use logged_stream::Record;
use logged_stream::RecordFilter;
use logged_stream::RecordKind;
use logged_stream::RecordKindFilter;

/// An event carrying a record built from `kind` and `message`.
fn event(kind: RecordKind, message: &str) -> Event<'static> {
    Event::from(Record::new(kind, String::from(message)))
}

fn criterion_benchmark(c: &mut Criterion) {
    let record_kind_filter = RecordKindFilter::new(&[RecordKind::Read]);

    c.bench_function("RecordKindFilter", move |b| {
        b.iter(|| {
            record_kind_filter.check(&event(RecordKind::Open, "open"));
            record_kind_filter.check(&event(RecordKind::Read, "read"));
            record_kind_filter.check(&event(RecordKind::Write, "write"));
            record_kind_filter.check(&event(RecordKind::Error, "error"));
            record_kind_filter.check(&event(RecordKind::Shutdown, "shutdown"));
            record_kind_filter.check(&event(RecordKind::Drop, "drop"));
        })
    });

    let all_filter = AllFilter::new(vec![
        Box::new(RecordKindFilter::new(&[
            RecordKind::Read,
            RecordKind::Write,
        ])),
        Box::new(RecordKindFilter::new(&[
            RecordKind::Read,
            RecordKind::Error,
        ])),
    ]);

    c.bench_function("AllFilter", move |b| {
        b.iter(|| {
            all_filter.check(&event(RecordKind::Open, "open"));
            all_filter.check(&event(RecordKind::Read, "read"));
            all_filter.check(&event(RecordKind::Write, "write"));
            all_filter.check(&event(RecordKind::Error, "error"));
            all_filter.check(&event(RecordKind::Shutdown, "shutdown"));
            all_filter.check(&event(RecordKind::Drop, "drop"));
        })
    });

    let any_filter = AnyFilter::new(vec![
        Box::new(RecordKindFilter::new(&[RecordKind::Read])),
        Box::new(RecordKindFilter::new(&[RecordKind::Write])),
    ]);

    c.bench_function("AnyFilter", move |b| {
        b.iter(|| {
            any_filter.check(&event(RecordKind::Open, "open"));
            any_filter.check(&event(RecordKind::Read, "read"));
            any_filter.check(&event(RecordKind::Write, "write"));
            any_filter.check(&event(RecordKind::Error, "error"));
            any_filter.check(&event(RecordKind::Shutdown, "shutdown"));
            any_filter.check(&event(RecordKind::Drop, "drop"));
        })
    });

    let nested_filter = AllFilter::new(vec![Box::new(AnyFilter::new(vec![
        Box::new(RecordKindFilter::new(&[RecordKind::Read])),
        Box::new(RecordKindFilter::new(&[RecordKind::Write])),
    ]))]);

    c.bench_function("NestedCompositeFilter", move |b| {
        b.iter(|| {
            nested_filter.check(&event(RecordKind::Open, "open"));
            nested_filter.check(&event(RecordKind::Read, "read"));
            nested_filter.check(&event(RecordKind::Write, "write"));
            nested_filter.check(&event(RecordKind::Error, "error"));
            nested_filter.check(&event(RecordKind::Shutdown, "shutdown"));
            nested_filter.check(&event(RecordKind::Drop, "drop"));
        })
    });
}

criterion_group! {
    name = filter;
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
criterion_main!(filter);
