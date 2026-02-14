use criterion::criterion_group;
use criterion::criterion_main;
use criterion::Criterion;
use logged_stream::AllFilter;
use logged_stream::AnyFilter;
use logged_stream::Record;
use logged_stream::RecordFilter;
use logged_stream::RecordKind;
use logged_stream::RecordKindFilter;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("RecordKindFilter", |b| {
        let record_kind_filter = RecordKindFilter::new(&[RecordKind::Read]);
        b.iter(|| {
            record_kind_filter.check(&Record::new(RecordKind::Open, String::from("open")));
            record_kind_filter.check(&Record::new(RecordKind::Read, String::from("read")));
            record_kind_filter.check(&Record::new(RecordKind::Write, String::from("write")));
            record_kind_filter.check(&Record::new(RecordKind::Error, String::from("error")));
            record_kind_filter.check(&Record::new(RecordKind::Shutdown, String::from("shutdown")));
            record_kind_filter.check(&Record::new(RecordKind::Drop, String::from("drop")));
        })
    });

    c.bench_function("AllFilter", |b| {
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
        b.iter(|| {
            all_filter.check(&Record::new(RecordKind::Open, String::from("open")));
            all_filter.check(&Record::new(RecordKind::Read, String::from("read")));
            all_filter.check(&Record::new(RecordKind::Write, String::from("write")));
            all_filter.check(&Record::new(RecordKind::Error, String::from("error")));
            all_filter.check(&Record::new(RecordKind::Shutdown, String::from("shutdown")));
            all_filter.check(&Record::new(RecordKind::Drop, String::from("drop")));
        })
    });

    c.bench_function("AnyFilter", |b| {
        let any_filter = AnyFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(RecordKindFilter::new(&[RecordKind::Write])),
        ]);
        b.iter(|| {
            any_filter.check(&Record::new(RecordKind::Open, String::from("open")));
            any_filter.check(&Record::new(RecordKind::Read, String::from("read")));
            any_filter.check(&Record::new(RecordKind::Write, String::from("write")));
            any_filter.check(&Record::new(RecordKind::Error, String::from("error")));
            any_filter.check(&Record::new(RecordKind::Shutdown, String::from("shutdown")));
            any_filter.check(&Record::new(RecordKind::Drop, String::from("drop")));
        })
    });

    c.bench_function("NestedCompositeFilter", |b| {
        let nested_filter = AllFilter::new(vec![Box::new(AnyFilter::new(vec![
            Box::new(RecordKindFilter::new(&[RecordKind::Read])),
            Box::new(RecordKindFilter::new(&[RecordKind::Write])),
        ]))]);
        b.iter(|| {
            nested_filter.check(&Record::new(RecordKind::Open, String::from("open")));
            nested_filter.check(&Record::new(RecordKind::Read, String::from("read")));
            nested_filter.check(&Record::new(RecordKind::Write, String::from("write")));
            nested_filter.check(&Record::new(RecordKind::Error, String::from("error")));
            nested_filter.check(&Record::new(RecordKind::Shutdown, String::from("shutdown")));
            nested_filter.check(&Record::new(RecordKind::Drop, String::from("drop")));
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
