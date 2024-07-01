use criterion::criterion_group;
use criterion::criterion_main;
use criterion::Criterion;
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
}

criterion_group! {
    name = filter;
    config = Criterion::default();
    targets = criterion_benchmark
}
criterion_main!(filter);
