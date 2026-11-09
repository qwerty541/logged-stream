//! The provided filters: `DefaultFilter`, `EventKindFilter`, the composites `AllFilter` and
//! `AnyFilter` (empty, nested, child order, formatting once for several children, mixed
//! formatted and raw children), deciding by kind without formatting, `Box` forwarding, and the
//! trait bounds and `Debug` output.

use super::helpers::CountingFilter;
use super::helpers::CountingFormatter;
use super::helpers::assert_send;
use super::helpers::assert_unpin;
use crate::buffer_formatter::LowercaseHexadecimalFormatter;
use crate::event::Event;
use crate::event::EventKind;
use crate::filter::AllFilter;
use crate::filter::AnyFilter;
use crate::filter::DefaultFilter;
use crate::filter::EventFilter;
use crate::filter::EventFilterDebug;
use crate::filter::EventKindFilter;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

/// Judges the formatted record: keeps events whose message contains the needle.
struct MessageFilter(&'static str);

impl EventFilter for MessageFilter {
    fn check(&self, event: &Event<'_>) -> bool {
        event.record().message.contains(self.0)
    }
}

/// Judges reads and writes on their first raw byte, and keeps every other event.
struct FirstByteFilter(u8);

impl EventFilter for FirstByteFilter {
    fn check(&self, event: &Event<'_>) -> bool {
        match event.bytes() {
            Some(bytes) => bytes.first() == Some(&self.0),
            None => true,
        }
    }
}

#[test]
fn test_unpin() {
    assert_unpin::<DefaultFilter>();
    assert_unpin::<EventKindFilter>();
    assert_unpin::<AllFilter>();
    assert_unpin::<AnyFilter>();
}

#[test]
fn test_default_filter() {
    assert!(DefaultFilter.check(&Event::message(EventKind::Read, "01:02:03:04:05:06")));
    assert!(DefaultFilter.check(&Event::message(EventKind::Write, "01:02:03:04:05:06")));
    assert!(DefaultFilter.check(&Event::message(EventKind::Drop, "deallocated")));
    assert!(DefaultFilter.check(&Event::message(
        EventKind::Shutdown,
        "write shutdown request"
    )));
}

#[test]
fn test_event_kind_filter() {
    let filter = EventKindFilter::new(&[EventKind::Read]);
    assert!(filter.check(&Event::message(EventKind::Read, "01:02:03:04:05:06")));
    assert!(!filter.check(&Event::message(EventKind::Write, "01:02:03:04:05:06")));
    assert!(!filter.check(&Event::message(EventKind::Drop, "deallocated")));
    assert!(!filter.check(&Event::message(
        EventKind::Shutdown,
        "write shutdown request"
    )));
}

#[test]
fn test_all_filter_empty() {
    let filter = AllFilter::new(vec![]);
    // Empty conjunction should return true (vacuously true)
    assert!(filter.check(&Event::message(EventKind::Read, "test")));
    assert!(filter.check(&Event::message(EventKind::Write, "test")));
    assert!(filter.check(&Event::message(EventKind::Error, "test")));
}

#[test]
fn test_all_filter_single() {
    let filter = AllFilter::new(vec![Box::new(EventKindFilter::new(&[EventKind::Read]))]);

    assert!(filter.check(&Event::message(EventKind::Read, "data")));
    assert!(!filter.check(&Event::message(EventKind::Write, "data")));
    assert!(!filter.check(&Event::message(EventKind::Error, "error")));
}

#[test]
fn test_all_filter_multiple_all_pass() {
    // Both filters accept Read
    let filter = AllFilter::new(vec![
        Box::new(EventKindFilter::new(&[EventKind::Read, EventKind::Write])),
        Box::new(EventKindFilter::new(&[EventKind::Read, EventKind::Error])),
    ]);

    assert!(filter.check(&Event::message(EventKind::Read, "data")));
}

#[test]
fn test_all_filter_multiple_one_fails() {
    // First accepts Write, second doesn't
    let filter = AllFilter::new(vec![
        Box::new(EventKindFilter::new(&[EventKind::Read, EventKind::Write])),
        Box::new(EventKindFilter::new(&[EventKind::Read, EventKind::Error])),
    ]);

    assert!(!filter.check(&Event::message(EventKind::Write, "data")));
}

#[test]
fn test_all_filter_multiple_all_fail() {
    let filter = AllFilter::new(vec![
        Box::new(EventKindFilter::new(&[EventKind::Read])),
        Box::new(EventKindFilter::new(&[EventKind::Write])),
    ]);

    assert!(!filter.check(&Event::message(EventKind::Error, "error")));
}

#[test]
fn test_all_filter_with_default() {
    // Combining with DefaultFilter (which always returns true)
    let filter = AllFilter::new(vec![
        Box::new(DefaultFilter),
        Box::new(EventKindFilter::new(&[EventKind::Read])),
    ]);

    assert!(filter.check(&Event::message(EventKind::Read, "data")));
    assert!(!filter.check(&Event::message(EventKind::Write, "data")));
}

#[test]
fn test_any_filter_empty() {
    let filter = AnyFilter::new(vec![]);
    // Empty disjunction should return false
    assert!(!filter.check(&Event::message(EventKind::Read, "test")));
    assert!(!filter.check(&Event::message(EventKind::Write, "test")));
    assert!(!filter.check(&Event::message(EventKind::Error, "test")));
}

#[test]
fn test_any_filter_single() {
    let filter = AnyFilter::new(vec![Box::new(EventKindFilter::new(&[EventKind::Read]))]);

    assert!(filter.check(&Event::message(EventKind::Read, "data")));
    assert!(!filter.check(&Event::message(EventKind::Write, "data")));
    assert!(!filter.check(&Event::message(EventKind::Error, "error")));
}

#[test]
fn test_any_filter_multiple_first_passes() {
    let filter = AnyFilter::new(vec![
        Box::new(EventKindFilter::new(&[EventKind::Read])),
        Box::new(EventKindFilter::new(&[EventKind::Write])),
    ]);

    assert!(filter.check(&Event::message(EventKind::Read, "data")));
}

#[test]
fn test_any_filter_multiple_second_passes() {
    let filter = AnyFilter::new(vec![
        Box::new(EventKindFilter::new(&[EventKind::Read])),
        Box::new(EventKindFilter::new(&[EventKind::Write])),
    ]);

    assert!(filter.check(&Event::message(EventKind::Write, "data")));
}

#[test]
fn test_any_filter_multiple_all_pass() {
    let filter = AnyFilter::new(vec![
        Box::new(EventKindFilter::new(&[EventKind::Read, EventKind::Write])),
        Box::new(EventKindFilter::new(&[EventKind::Read, EventKind::Error])),
    ]);

    assert!(filter.check(&Event::message(EventKind::Read, "data")));
}

#[test]
fn test_any_filter_multiple_all_fail() {
    let filter = AnyFilter::new(vec![
        Box::new(EventKindFilter::new(&[EventKind::Read])),
        Box::new(EventKindFilter::new(&[EventKind::Write])),
    ]);

    assert!(!filter.check(&Event::message(EventKind::Error, "error")));
}

#[test]
fn test_any_filter_with_default() {
    // Combining with DefaultFilter (which always returns true)
    let filter = AnyFilter::new(vec![
        Box::new(EventKindFilter::new(&[EventKind::Read])),
        Box::new(DefaultFilter),
    ]);

    // Should pass for everything because DefaultFilter always returns true
    assert!(filter.check(&Event::message(EventKind::Read, "data")));
    assert!(filter.check(&Event::message(EventKind::Write, "data")));
    assert!(filter.check(&Event::message(EventKind::Error, "error")));
}

#[test]
fn test_nested_composite_filters() {
    // (Read OR Write)
    // Implemented as: AllFilter containing AnyFilter for (Read OR Write)
    let filter = AllFilter::new(vec![Box::new(AnyFilter::new(vec![
        Box::new(EventKindFilter::new(&[EventKind::Read])),
        Box::new(EventKindFilter::new(&[EventKind::Write])),
    ]))]);

    assert!(filter.check(&Event::message(EventKind::Read, "data")));
    assert!(filter.check(&Event::message(EventKind::Write, "data")));
    assert!(!filter.check(&Event::message(EventKind::Drop, "dropped")));
    assert!(!filter.check(&Event::message(EventKind::Error, "error")));
}

#[test]
fn test_complex_nested_filters() {
    // AllFilter containing two AnyFilters:
    // (Read OR Write) AND (Read OR Error)
    // This should only pass for Read
    let filter = AllFilter::new(vec![
        Box::new(AnyFilter::new(vec![
            Box::new(EventKindFilter::new(&[EventKind::Read])),
            Box::new(EventKindFilter::new(&[EventKind::Write])),
        ])),
        Box::new(AnyFilter::new(vec![
            Box::new(EventKindFilter::new(&[EventKind::Read])),
            Box::new(EventKindFilter::new(&[EventKind::Error])),
        ])),
    ]);

    assert!(filter.check(&Event::message(EventKind::Read, "data")));
    assert!(!filter.check(&Event::message(EventKind::Write, "data")));
    assert!(!filter.check(&Event::message(EventKind::Error, "error")));
    assert!(!filter.check(&Event::message(EventKind::Drop, "dropped")));
}

#[test]
fn test_provided_filters_decide_by_kind_without_formatting() {
    let kinds = [
        EventKind::Open,
        EventKind::Read,
        EventKind::Write,
        EventKind::Error,
        EventKind::Shutdown,
        EventKind::Drop,
    ];
    let filters: Vec<Box<dyn EventFilter>> = vec![
        Box::new(DefaultFilter),
        Box::new(EventKindFilter::new(&[])),
        Box::new(EventKindFilter::new(&[EventKind::Read])),
        Box::new(EventKindFilter::new(&[
            EventKind::Drop,
            EventKind::Error,
            EventKind::Shutdown,
        ])),
        Box::new(AllFilter::new(vec![])),
        Box::new(AnyFilter::new(vec![])),
        Box::new(AllFilter::new(vec![
            Box::new(EventKindFilter::new(&[EventKind::Read, EventKind::Write])),
            Box::new(EventKindFilter::new(&[EventKind::Read, EventKind::Error])),
        ])),
        Box::new(AnyFilter::new(vec![
            Box::new(EventKindFilter::new(&[EventKind::Read])),
            Box::new(EventKindFilter::new(&[EventKind::Write])),
        ])),
        Box::new(AllFilter::new(vec![Box::new(AnyFilter::new(vec![
            Box::new(DefaultFilter),
            Box::new(EventKindFilter::new(&[EventKind::Write])),
        ]))])),
    ];
    let formatter = CountingFormatter::default();

    for filter in &filters {
        for kind in kinds {
            // The event `LoggedStream` builds: bytes for a read or write, a message for anything else.
            let built = match kind {
                EventKind::Read => Event::read(&[0x01, 0x02], &formatter),
                EventKind::Write => Event::write(&[0x01, 0x02], &formatter),
                _ => Event::message(kind, "message"),
            };
            assert_eq!(
                filter.check(&built),
                filter.check(&Event::message(kind, "01:02")),
                "{:?} judges more than the kind of {kind:?}",
                EventFilterDebug(filter.as_ref())
            );
        }
    }
    // They decide from the kind alone, so nothing was formatted.
    assert_eq!(formatter.0.load(Ordering::SeqCst), 0);
}

#[test]
fn test_box_forwards_check() {
    let formatter = LowercaseHexadecimalFormatter::new_default();
    let handshake = Event::read(&[0x16, 0x03], &formatter);
    let application_data = Event::read(&[0x17, 0x03], &formatter);

    // The filter decides on the raw bytes, so each wrapper has to hand on the event it was given.
    let boxed: Box<dyn EventFilter> = Box::new(FirstByteFilter(0x16));
    assert!(boxed.check(&handshake));
    assert!(!boxed.check(&application_data));

    let double_boxed: Box<Box<dyn EventFilter>> = Box::new(boxed);
    assert!(!double_boxed.check(&application_data));

    // Composite children are boxed too.
    let all = AllFilter::new(vec![Box::new(FirstByteFilter(0x16))]);
    assert!(!all.check(&application_data));
    let any = AnyFilter::new(vec![Box::new(FirstByteFilter(0x16))]);
    assert!(!any.check(&application_data));
}

#[test]
fn test_composite_formats_once_for_several_children() {
    let formatter = CountingFormatter::default();
    let read = Event::read(&[0x0a, 0xff], &formatter);

    let filter = AllFilter::new(vec![
        Box::new(MessageFilter("0a")),
        Box::new(MessageFilter("ff")),
    ]);
    assert!(filter.check(&read));
    assert_eq!(formatter.0.load(Ordering::SeqCst), 1);
}

#[test]
fn test_composite_keeps_child_order() {
    // A child that asks for the formatted record comes before a kind filter: it still sees the write, and the
    // write is formatted for it.
    let calls = Arc::new(AtomicUsize::new(0));
    let formatter = CountingFormatter::default();
    let write = Event::write(&[0x01], &formatter);
    let filter = AllFilter::new(vec![
        Box::new(CountingFilter(Arc::clone(&calls))),
        Box::new(EventKindFilter::new(&[EventKind::Read])),
    ]);
    assert!(!filter.check(&write));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(formatter.0.load(Ordering::SeqCst), 1);

    // The kind filter comes first: the write is rejected before the child or the formatter runs.
    let calls = Arc::new(AtomicUsize::new(0));
    let formatter = CountingFormatter::default();
    let write = Event::write(&[0x01], &formatter);
    let filter = AllFilter::new(vec![
        Box::new(EventKindFilter::new(&[EventKind::Read])),
        Box::new(CountingFilter(Arc::clone(&calls))),
    ]);
    assert!(!filter.check(&write));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(formatter.0.load(Ordering::SeqCst), 0);

    // AnyFilter mirrors it: a kind filter that accepts settles the read before the child runs.
    let calls = Arc::new(AtomicUsize::new(0));
    let formatter = CountingFormatter::default();
    let read = Event::read(&[0x01], &formatter);
    let filter = AnyFilter::new(vec![
        Box::new(EventKindFilter::new(&[EventKind::Read])),
        Box::new(CountingFilter(Arc::clone(&calls))),
    ]);
    assert!(filter.check(&read));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(formatter.0.load(Ordering::SeqCst), 0);
}

#[test]
fn test_composite_mixes_formatted_and_raw_children() {
    // The first child judges the formatted message and accepts; the raw child after it still decides on the
    // raw bytes. Each event is formatted once, for the first child.
    let formatter = CountingFormatter::default();
    let all = AllFilter::new(vec![
        Box::new(MessageFilter("16")),
        Box::new(FirstByteFilter(0x16)),
    ]);
    assert!(all.check(&Event::read(&[0x16, 0x01], &formatter)));
    assert!(!all.check(&Event::read(&[0x01, 0x16], &formatter)));
    assert_eq!(formatter.0.load(Ordering::SeqCst), 2);

    // AnyFilter mirrors it: the first child rejects on the formatted message, the raw child decides.
    let formatter = CountingFormatter::default();
    let any = AnyFilter::new(vec![
        Box::new(MessageFilter("ff")),
        Box::new(FirstByteFilter(0x16)),
    ]);
    assert!(any.check(&Event::read(&[0x16, 0x01], &formatter)));
    assert!(!any.check(&Event::read(&[0x01, 0x02], &formatter)));
    assert_eq!(formatter.0.load(Ordering::SeqCst), 2);
}

#[test]
fn test_trait_object_safety() {
    // Assert trait object construct.
    let default: Box<dyn EventFilter> = Box::<DefaultFilter>::default();
    let event_kind: Box<dyn EventFilter> = Box::new(EventKindFilter::new(&[]));
    let all: Box<dyn EventFilter> = Box::new(AllFilter::new(vec![]));
    let any: Box<dyn EventFilter> = Box::new(AnyFilter::new(vec![]));

    let open = Event::message(EventKind::Open, "test log record");

    // Assert that trait object methods are dispatchable.
    _ = default.check(&open);
    _ = event_kind.check(&open);
    _ = all.check(&open);
    _ = any.check(&open);

    let formatter = LowercaseHexadecimalFormatter::new_default();
    let read = Event::read(&[0x01], &formatter);
    assert!(default.check(&read));
    assert!(!event_kind.check(&read));
    assert!(all.check(&read));
    assert!(!any.check(&read));
}

fn assert_event_filter<T: EventFilter>() {}

#[test]
fn test_box() {
    assert_event_filter::<Box<dyn EventFilter>>();
    assert_event_filter::<Box<EventKindFilter>>();
    assert_event_filter::<Box<DefaultFilter>>();
    assert_event_filter::<Box<AllFilter>>();
    assert_event_filter::<Box<AnyFilter>>();
}

#[test]
fn test_send() {
    assert_send::<EventKindFilter>();
    assert_send::<DefaultFilter>();
    assert_send::<AllFilter>();
    assert_send::<AnyFilter>();

    assert_send::<Box<dyn EventFilter>>();
    assert_send::<Box<EventKindFilter>>();
    assert_send::<Box<DefaultFilter>>();
    assert_send::<Box<AllFilter>>();
    assert_send::<Box<AnyFilter>>();
}

fn assert_debug<T: fmt::Debug>() {}

#[test]
fn test_debug() {
    assert_debug::<DefaultFilter>();
    assert_debug::<EventKindFilter>();
    assert_debug::<AllFilter>();
    assert_debug::<AnyFilter>();

    assert_debug::<Box<DefaultFilter>>();
    assert_debug::<Box<EventKindFilter>>();
    assert_debug::<Box<AllFilter>>();
    assert_debug::<Box<AnyFilter>>();
}

#[test]
fn test_debug_output() {
    // DefaultFilter
    assert_eq!(format!("{:?}", DefaultFilter), "DefaultFilter");

    // EventKindFilter
    let event_kind_filter = EventKindFilter::new(&[EventKind::Read, EventKind::Write]);
    let debug_str = format!("{:?}", event_kind_filter);
    assert!(debug_str.contains("EventKindFilter"));
    assert!(debug_str.contains("Read"));
    assert!(debug_str.contains("Write"));

    // AllFilter (empty)
    let all_filter_empty = AllFilter::new(vec![]);
    let debug_str = format!("{:?}", all_filter_empty);
    assert!(debug_str.contains("AllFilter"));
    assert!(debug_str.contains("filters: []"));

    // AllFilter (with children)
    let all_filter = AllFilter::new(vec![
        Box::new(EventKindFilter::new(&[EventKind::Read])),
        Box::new(DefaultFilter),
    ]);
    let debug_str = format!("{:?}", all_filter);
    assert!(debug_str.contains("AllFilter"));
    assert!(debug_str.contains("EventKindFilter"));
    assert!(debug_str.contains("Read"));
    assert!(debug_str.contains("DefaultFilter"));

    // AnyFilter (empty)
    let any_filter_empty = AnyFilter::new(vec![]);
    let debug_str = format!("{:?}", any_filter_empty);
    assert!(debug_str.contains("AnyFilter"));
    assert!(debug_str.contains("filters: []"));

    // AnyFilter (with children)
    let any_filter = AnyFilter::new(vec![
        Box::new(EventKindFilter::new(&[EventKind::Write])),
        Box::new(DefaultFilter),
    ]);
    let debug_str = format!("{:?}", any_filter);
    assert!(debug_str.contains("AnyFilter"));
    assert!(debug_str.contains("EventKindFilter"));
    assert!(debug_str.contains("Write"));
    assert!(debug_str.contains("DefaultFilter"));

    // Nested composite filter
    let nested = AllFilter::new(vec![Box::new(AnyFilter::new(vec![Box::new(
        EventKindFilter::new(&[EventKind::Read]),
    )]))]);
    let debug_str = format!("{:?}", nested);
    assert!(debug_str.contains("AllFilter"));
    assert!(debug_str.contains("AnyFilter"));
    assert!(debug_str.contains("EventKindFilter"));
    assert!(debug_str.contains("Read"));
}
