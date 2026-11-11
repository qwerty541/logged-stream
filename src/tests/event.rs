//! `Event`: its kind and bytes, its record (formatted at most once, and only on demand), events
//! made from a message, and a `Debug` that does not format.

use super::helpers::CountingFormatter;
use crate::Event;
use crate::EventKind;
use std::sync::atomic::Ordering;

#[test]
fn event_exposes_kind_and_bytes_without_formatting() {
    let formatter = CountingFormatter::default();
    let read = Event::read(&[0x0a, 0xff], &formatter);
    let write = Event::write(&[0x01], &formatter);

    assert_eq!(read.kind(), EventKind::Read);
    assert_eq!(read.bytes(), Some(&[0x0a, 0xff][..]));
    assert_eq!(write.kind(), EventKind::Write);
    assert_eq!(write.bytes(), Some(&[0x01][..]));
    assert_eq!(formatter.0.load(Ordering::SeqCst), 0);
}

#[test]
fn event_formats_once_and_reuses_the_record() {
    let formatter = CountingFormatter::default();
    let event = Event::write(&[0x0a, 0xff], &formatter);

    let first = event.record().clone();
    assert_eq!(first.kind, EventKind::Write);
    assert_eq!(first.message, "0a:ff");
    assert_eq!(event.record(), &first);

    // The record handed on afterwards is the one already built, timestamp included.
    assert_eq!(event.into_record(), first);
    assert_eq!(formatter.0.load(Ordering::SeqCst), 1);
}

#[test]
fn event_into_record_formats_when_nothing_asked() {
    let formatter = CountingFormatter::default();
    let event = Event::read(&[0x01, 0x02], &formatter);

    let record = event.into_record();
    assert_eq!(record.kind, EventKind::Read);
    assert_eq!(record.message, "01:02");
    assert_eq!(formatter.0.load(Ordering::SeqCst), 1);
}

#[test]
fn event_message_has_no_bytes_and_carries_the_message() {
    let event = Event::message(EventKind::Error, "Error during read: boom");

    assert_eq!(event.kind(), EventKind::Error);
    assert_eq!(event.bytes(), None);

    let record = event.record().clone();
    assert_eq!(record.kind, EventKind::Error);
    assert_eq!(record.message, "Error during read: boom");
    // The record handed on afterwards is the one already built, timestamp included.
    assert_eq!(event.into_record(), record);

    // An owned message works too, and even a read built this way has no bytes.
    let read = Event::message(EventKind::Read, String::from("01:02"));
    assert_eq!(read.kind(), EventKind::Read);
    assert_eq!(read.bytes(), None);
    assert_eq!(read.record().message, "01:02");
}

#[test]
fn event_debug_does_not_format() {
    let formatter = CountingFormatter::default();
    let event = Event::read(&[0x01], &formatter);

    let debug = format!("{event:?}");
    assert!(debug.contains("Event"));
    assert!(debug.contains("Read"));
    assert!(debug.contains("bytes: Some([1])"));
    assert!(debug.contains("record: None"));
    assert_eq!(formatter.0.load(Ordering::SeqCst), 0);

    // Once formatted, the record is shown.
    event.record();
    assert!(format!("{event:?}").contains("message: \"01\""));

    let deallocated = Event::message(EventKind::Drop, "Deallocated.");
    let debug = format!("{deallocated:?}");
    assert!(debug.contains("bytes: None"));
    assert!(debug.contains("message: \"Deallocated.\""));
}
