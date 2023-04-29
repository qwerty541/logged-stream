use crate::Record;
use crate::RecordKind;
use itertools::Itertools;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// Trait
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

pub trait RecordFilter: 'static {
    fn check(&self, record: &Record) -> bool;
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// DefaultFilter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultFilter;

impl RecordFilter for DefaultFilter {
    fn check(&self, _record: &Record) -> bool {
        true
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// RecordKindFilter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct RecordKindFilter {
    allowed_kinds: Vec<RecordKind>,
}

impl RecordKindFilter {
    pub fn new(kinds: &'static [RecordKind]) -> Self {
        Self {
            allowed_kinds: kinds.iter().copied().unique().collect(),
        }
    }
}

impl RecordFilter for RecordKindFilter {
    fn check(&self, record: &Record) -> bool {
        self.allowed_kinds.contains(&record.kind)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// Tests
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use crate::filter::DefaultFilter;
    use crate::filter::RecordFilter;
    use crate::filter::RecordKindFilter;
    use crate::record::Record;
    use crate::record::RecordKind;
    use std::convert::From;
    use std::marker::Unpin;

    fn assert_unpin<T: Unpin>() {}

    #[test]
    fn test_unpin() {
        assert_unpin::<DefaultFilter>();
        assert_unpin::<RecordKindFilter>();
    }

    #[test]
    fn test_default_filter() {
        let filter = DefaultFilter::default();
        assert!(filter.check(&Record::new(
            RecordKind::Read,
            String::from("01:02:03:04:05:06")
        )));
        assert!(filter.check(&Record::new(
            RecordKind::Write,
            String::from("01:02:03:04:05:06")
        )));
        assert!(filter.check(&Record::new(RecordKind::Drop, String::from("deallocated"))));
        assert!(filter.check(&Record::new(
            RecordKind::Shutdown,
            String::from("write shutdown request")
        )));
    }

    #[test]
    fn test_record_kind_filter() {
        let filter = RecordKindFilter::new(&[RecordKind::Read]);
        assert!(filter.check(&Record::new(
            RecordKind::Read,
            String::from("01:02:03:04:05:06")
        )));
        assert!(!filter.check(&Record::new(
            RecordKind::Write,
            String::from("01:02:03:04:05:06")
        )));
        assert!(!filter.check(&Record::new(RecordKind::Drop, String::from("deallocated"))));
        assert!(!filter.check(&Record::new(
            RecordKind::Shutdown,
            String::from("write shutdown request")
        )));
    }

    #[test]
    fn test_trait_object_safety() {
        // Assert traint object construct.
        let default: Box<dyn RecordFilter> = Box::<DefaultFilter>::default();
        let record_kind: Box<dyn RecordFilter> = Box::new(RecordKindFilter::new(&[]));

        let record = Record::new(RecordKind::Open, String::from("test log record"));

        // Assert that trait object methods are dispatchable.
        _ = default.check(&record);
        _ = record_kind.check(&record);
    }
}
