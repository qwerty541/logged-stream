use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use logged_stream::AllFilter;
use logged_stream::AnyFilter;
use logged_stream::Event;
use logged_stream::EventFilter;
use logged_stream::EventKind;
use logged_stream::EventKindFilter;

fn criterion_benchmark(c: &mut Criterion) {
    let event_kind_filter = EventKindFilter::new(&[EventKind::Read]);

    c.bench_function("EventKindFilter", move |b| {
        b.iter(|| {
            event_kind_filter.check(&Event::message(EventKind::Open, "open"));
            event_kind_filter.check(&Event::message(EventKind::Read, "read"));
            event_kind_filter.check(&Event::message(EventKind::Write, "write"));
            event_kind_filter.check(&Event::message(EventKind::Error, "error"));
            event_kind_filter.check(&Event::message(EventKind::Shutdown, "shutdown"));
            event_kind_filter.check(&Event::message(EventKind::Drop, "drop"));
        })
    });

    let all_filter = AllFilter::new(vec![
        Box::new(EventKindFilter::new(&[EventKind::Read, EventKind::Write])),
        Box::new(EventKindFilter::new(&[EventKind::Read, EventKind::Error])),
    ]);

    c.bench_function("AllFilter", move |b| {
        b.iter(|| {
            all_filter.check(&Event::message(EventKind::Open, "open"));
            all_filter.check(&Event::message(EventKind::Read, "read"));
            all_filter.check(&Event::message(EventKind::Write, "write"));
            all_filter.check(&Event::message(EventKind::Error, "error"));
            all_filter.check(&Event::message(EventKind::Shutdown, "shutdown"));
            all_filter.check(&Event::message(EventKind::Drop, "drop"));
        })
    });

    let any_filter = AnyFilter::new(vec![
        Box::new(EventKindFilter::new(&[EventKind::Read])),
        Box::new(EventKindFilter::new(&[EventKind::Write])),
    ]);

    c.bench_function("AnyFilter", move |b| {
        b.iter(|| {
            any_filter.check(&Event::message(EventKind::Open, "open"));
            any_filter.check(&Event::message(EventKind::Read, "read"));
            any_filter.check(&Event::message(EventKind::Write, "write"));
            any_filter.check(&Event::message(EventKind::Error, "error"));
            any_filter.check(&Event::message(EventKind::Shutdown, "shutdown"));
            any_filter.check(&Event::message(EventKind::Drop, "drop"));
        })
    });

    let nested_filter = AllFilter::new(vec![Box::new(AnyFilter::new(vec![
        Box::new(EventKindFilter::new(&[EventKind::Read])),
        Box::new(EventKindFilter::new(&[EventKind::Write])),
    ]))]);

    c.bench_function("NestedCompositeFilter", move |b| {
        b.iter(|| {
            nested_filter.check(&Event::message(EventKind::Open, "open"));
            nested_filter.check(&Event::message(EventKind::Read, "read"));
            nested_filter.check(&Event::message(EventKind::Write, "write"));
            nested_filter.check(&Event::message(EventKind::Error, "error"));
            nested_filter.check(&Event::message(EventKind::Shutdown, "shutdown"));
            nested_filter.check(&Event::message(EventKind::Drop, "drop"));
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
