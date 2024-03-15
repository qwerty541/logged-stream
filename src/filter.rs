use crate::Record;
use crate::RecordKind;
use itertools::Itertools;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Trait
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// This trait allows to filter log records ([`Record`]) using [`check`] method which returns [`bool`] value.
/// It should be implemented for structures which are going to be used as filtering part inside [`LoggedStream`].
///
/// [`check`]: RecordFilter::check
/// [`LoggedStream`]: crate::LoggedStream
pub trait RecordFilter: Send + 'static {
    /// This method returns [`bool`] value depending on if received log record ([`Record`]) should be processed
    /// by logging part inside [`LoggedStream`].
    ///
    /// [`LoggedStream`]: crate::LoggedStream
    fn check(&self, record: &Record) -> bool;
}

impl RecordFilter for Box<dyn RecordFilter> {
    fn check(&self, record: &Record) -> bool {
        (**self).check(record)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// DefaultFilter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// This is default implementation of [`RecordFilter`] trait which [`check`] method always return `true`.
/// It should be constructed using [`Default::default`] method.
///
/// [`check`]: RecordFilter::check
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultFilter;

impl RecordFilter for DefaultFilter {
    fn check(&self, _record: &Record) -> bool {
        true
    }
}

impl RecordFilter for Box<DefaultFilter> {
    fn check(&self, record: &Record) -> bool {
        (**self).check(record)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// RecordKindFilter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// This implementation of [`RecordFilter`] trait accepts allowed log record kinds ([`RecordKind`]) array during
/// construction and its [`check`] method returns `true` if received log record kind presented inside this array.
///
/// [`check`]: RecordFilter::check
#[derive(Debug)]
pub struct RecordKindFilter {
    allowed_kinds: Vec<RecordKind>,
}

impl RecordKindFilter {
    /// Construct a new instance of [`RecordKindFilter`] using provided array of allowed log record kinds ([`RecordKind`]).
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

impl RecordFilter for Box<RecordKindFilter> {
    fn check(&self, record: &Record) -> bool {
        (**self).check(record)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Tests
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use crate::filter::DefaultFilter;
    use crate::filter::RecordFilter;
    use crate::filter::RecordKindFilter;
    use crate::record::Record;
    use crate::record::RecordKind;

    fn assert_unpin<T: Unpin>() {}

    #[test]
    fn test_unpin() {
        assert_unpin::<DefaultFilter>();
        assert_unpin::<RecordKindFilter>();
    }

    #[test]
    fn test_default_filter() {
        assert!(DefaultFilter.check(&Record::new(
            RecordKind::Read,
            String::from("01:02:03:04:05:06")
        )));
        assert!(DefaultFilter.check(&Record::new(
            RecordKind::Write,
            String::from("01:02:03:04:05:06")
        )));
        assert!(DefaultFilter.check(&Record::new(RecordKind::Drop, String::from("deallocated"))));
        assert!(DefaultFilter.check(&Record::new(
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

    fn assert_record_filter<T: RecordFilter>() {}

    #[test]
    fn test_box() {
        assert_record_filter::<Box<dyn RecordFilter>>();
        assert_record_filter::<Box<RecordKindFilter>>();
        assert_record_filter::<Box<DefaultFilter>>();
    }

    fn assert_send<T: Send>() {}

    #[test]
    fn test_send() {
        assert_send::<RecordKindFilter>();
        assert_send::<DefaultFilter>();

        assert_send::<Box<dyn RecordFilter>>();
        assert_send::<Box<RecordKindFilter>>();
        assert_send::<Box<DefaultFilter>>();
    }
}
