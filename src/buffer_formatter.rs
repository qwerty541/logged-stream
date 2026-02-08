use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

const DEFAULT_SEPARATOR: &str = ":";

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Trait
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// This trait allows to format bytes buffer using [`format_buffer`] method. It should be implemented for
/// structures which are going to be used as formatting part inside [`LoggedStream`].
///
/// [`format_buffer`]: BufferFormatter::format_buffer
/// [`LoggedStream`]: crate::LoggedStream
pub trait BufferFormatter: Send + 'static {
    /// This method returns a separator which will be inserted between bytes during [`format_buffer`] method call.
    /// It should be implemented manually.
    ///
    /// [`format_buffer`]: BufferFormatter::format_buffer
    fn get_separator(&self) -> &str;

    /// This method accepts one byte from buffer and format it into [`String`]. It should be implemeted manually.
    fn format_byte(&self, byte: &u8) -> String;

    /// This method accepts bytes buffer and format it into [`String`]. It is automatically implemented method.
    fn format_buffer(&self, buffer: &[u8]) -> String {
        buffer
            .iter()
            .map(|b| self.format_byte(b))
            .collect::<Vec<String>>()
            .join(self.get_separator())
    }
}

impl BufferFormatter for Box<dyn BufferFormatter> {
    #[inline]
    fn get_separator(&self) -> &str {
        (**self).get_separator()
    }

    #[inline]
    fn format_byte(&self, byte: &u8) -> String {
        (**self).format_byte(byte)
    }
}

impl<T: BufferFormatter + ?Sized + Sync> BufferFormatter for &'static T {
    #[inline]
    fn get_separator(&self) -> &str {
        (**self).get_separator()
    }

    #[inline]
    fn format_byte(&self, byte: &u8) -> String {
        (**self).format_byte(byte)
    }
}

impl<T: BufferFormatter + Sync> BufferFormatter for Arc<T> {
    #[inline]
    fn get_separator(&self) -> &str {
        (**self).get_separator()
    }

    #[inline]
    fn format_byte(&self, byte: &u8) -> String {
        (**self).format_byte(byte)
    }
}

impl BufferFormatter for Arc<dyn BufferFormatter + Sync> {
    #[inline]
    fn get_separator(&self) -> &str {
        (**self).get_separator()
    }

    #[inline]
    fn format_byte(&self, byte: &u8) -> String {
        (**self).format_byte(byte)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Macro for Formatter Generation
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

macro_rules! define_formatter {
    (
        $(#[$struct_meta:meta])*
        $name:ident,
        $format_expr:expr
    ) => {
        $(#[$struct_meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name {
            separator: Cow<'static, str>,
        }

        impl $name {
            /// Construct a new instance of
            #[doc = concat!("`", stringify!($name), "`")]
            /// using provided borrowed separator. In case if provided
            /// separator will be [`None`], then default separator (`:`) will be used.
            pub fn new(provided_separator: Option<&str>) -> Self {
                Self {
                    separator: provided_separator
                        .map(|s| Cow::Owned(s.to_string()))
                        .unwrap_or(Cow::Borrowed(DEFAULT_SEPARATOR)),
                }
            }

            /// Construct a new instance of
            #[doc = concat!("`", stringify!($name), "`")]
            /// using provided static borrowed separator. In case if provided
            /// separator will be [`None`], then default separator (`:`) will be used. This method avoids allocation for
            /// static string separators.
            pub fn new_static(provided_separator: Option<&'static str>) -> Self {
                Self {
                    separator: provided_separator
                        .map(Cow::Borrowed)
                        .unwrap_or(Cow::Borrowed(DEFAULT_SEPARATOR)),
                }
            }

            /// Construct a new instance of
            #[doc = concat!("`", stringify!($name), "`")]
            /// using provided owned separator. In case if provided
            /// separator will be [`None`], then default separator (`:`) will be used.
            pub fn new_owned(provided_separator: Option<String>) -> Self {
                Self {
                    separator: provided_separator
                        .map(Cow::Owned)
                        .unwrap_or(Cow::Borrowed(DEFAULT_SEPARATOR)),
                }
            }

            /// Construct a new instance of
            #[doc = concat!("`", stringify!($name), "`")]
            /// using default separator (`:`).
            pub fn new_default() -> Self {
                Self {
                    separator: Cow::Borrowed(DEFAULT_SEPARATOR),
                }
            }
        }

        impl BufferFormatter for $name {
            #[inline]
            fn get_separator(&self) -> &str {
                &self.separator
            }

            #[inline]
            fn format_byte(&self, byte: &u8) -> String {
                $format_expr(byte)
            }
        }

        impl BufferFormatter for Box<$name> {
            #[inline]
            fn get_separator(&self) -> &str {
                (**self).get_separator()
            }

            #[inline]
            fn format_byte(&self, byte: &u8) -> String {
                (**self).format_byte(byte)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new_default()
            }
        }

        impl From<Cow<'static, str>> for $name {
            fn from(separator: Cow<'static, str>) -> Self {
                Self { separator }
            }
        }

        impl From<&str> for $name {
            fn from(separator: &str) -> Self {
                Self::new(Some(separator))
            }
        }

        impl From<String> for $name {
            fn from(separator: String) -> Self {
                Self::new_owned(Some(separator))
            }
        }

        impl From<Option<&str>> for $name {
            fn from(separator: Option<&str>) -> Self {
                Self::new(separator)
            }
        }

        impl From<Option<String>> for $name {
            fn from(separator: Option<String>) -> Self {
                Self::new_owned(separator)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}(separator: {:?})", stringify!($name), self.separator)
            }
        }
    };
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Formatters definitions
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

define_formatter!(
    /// This implementation of [`BufferFormatter`] trait formats provided bytes buffer in decimal number system.
    DecimalFormatter,
    |byte: &u8| format!("{byte}")
);

define_formatter!(
    /// This implementation of [`BufferFormatter`] trait formats provided bytes buffer in octal number system.
    OctalFormatter,
    |byte: &u8| format!("{byte:03o}")
);

define_formatter!(
    /// This implementation of [`BufferFormatter`] trait formats provided bytes buffer in hexadecimal number system.
    UppercaseHexadecimalFormatter,
    |byte: &u8| format!("{byte:02X}")
);

define_formatter!(
    /// This implementation of [`BufferFormatter`] trait formats provided bytes buffer in hexadecimal number system.
    LowercaseHexadecimalFormatter,
    |byte: &u8| format!("{byte:02x}")
);

define_formatter!(
    /// This implementation of [`BufferFormatter`] trait formats provided bytes buffer in binary number system.
    BinaryFormatter,
    |byte: &u8| format!("{byte:08b}")
);

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Tests
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use crate::buffer_formatter::BinaryFormatter;
    use crate::buffer_formatter::BufferFormatter;
    use crate::buffer_formatter::DecimalFormatter;
    use crate::buffer_formatter::LowercaseHexadecimalFormatter;
    use crate::buffer_formatter::OctalFormatter;
    use crate::buffer_formatter::UppercaseHexadecimalFormatter;
    use std::borrow::Cow;
    use std::sync::Arc;
    use std::thread;

    const FORMATTING_TEST_VALUES: &[u8] = &[10, 11, 12, 13, 14, 15, 16, 17, 18];

    #[test]
    fn test_buffer_formatting() {
        let lowercase_hexadecimal = LowercaseHexadecimalFormatter::new_default();
        let uppercase_hexadecimal = UppercaseHexadecimalFormatter::new_default();
        let decimal = DecimalFormatter::new_default();
        let octal = OctalFormatter::new_default();
        let binary = BinaryFormatter::new_default();

        assert_eq!(
            lowercase_hexadecimal.format_buffer(FORMATTING_TEST_VALUES),
            String::from("0a:0b:0c:0d:0e:0f:10:11:12")
        );
        assert_eq!(
            uppercase_hexadecimal.format_buffer(FORMATTING_TEST_VALUES),
            String::from("0A:0B:0C:0D:0E:0F:10:11:12")
        );
        assert_eq!(
            decimal.format_buffer(FORMATTING_TEST_VALUES),
            String::from("10:11:12:13:14:15:16:17:18")
        );
        assert_eq!(
            octal.format_buffer(FORMATTING_TEST_VALUES),
            String::from("012:013:014:015:016:017:020:021:022")
        );
        assert_eq!(
            binary.format_buffer(FORMATTING_TEST_VALUES),
            String::from(
                "00001010:00001011:00001100:00001101:00001110:00001111:00010000:00010001:00010010"
            )
        );
    }

    #[test]
    fn test_custom_separator() {
        let lowercase_hexadecimal = LowercaseHexadecimalFormatter::new(Some("-"));
        let uppercase_hexadecimal = UppercaseHexadecimalFormatter::new(Some("-"));
        let decimal = DecimalFormatter::new(Some("-"));
        let octal = OctalFormatter::new(Some("-"));
        let binary = BinaryFormatter::new(Some("-"));

        assert_eq!(
            lowercase_hexadecimal.format_buffer(FORMATTING_TEST_VALUES),
            String::from("0a-0b-0c-0d-0e-0f-10-11-12")
        );
        assert_eq!(
            uppercase_hexadecimal.format_buffer(FORMATTING_TEST_VALUES),
            String::from("0A-0B-0C-0D-0E-0F-10-11-12")
        );
        assert_eq!(
            decimal.format_buffer(FORMATTING_TEST_VALUES),
            String::from("10-11-12-13-14-15-16-17-18")
        );
        assert_eq!(
            octal.format_buffer(FORMATTING_TEST_VALUES),
            String::from("012-013-014-015-016-017-020-021-022")
        );
        assert_eq!(
            binary.format_buffer(FORMATTING_TEST_VALUES),
            String::from(
                "00001010-00001011-00001100-00001101-00001110-00001111-00010000-00010001-00010010"
            )
        );
    }

    #[test]
    fn test_static_separator() {
        // new_static should produce same results as new, just without allocation
        let lowercase_hexadecimal = LowercaseHexadecimalFormatter::new_static(Some("-"));
        let uppercase_hexadecimal = UppercaseHexadecimalFormatter::new_static(Some("-"));
        let decimal = DecimalFormatter::new_static(Some("-"));
        let octal = OctalFormatter::new_static(Some("-"));
        let binary = BinaryFormatter::new_static(Some("-"));

        assert_eq!(
            lowercase_hexadecimal.format_buffer(FORMATTING_TEST_VALUES),
            String::from("0a-0b-0c-0d-0e-0f-10-11-12")
        );
        assert_eq!(
            uppercase_hexadecimal.format_buffer(FORMATTING_TEST_VALUES),
            String::from("0A-0B-0C-0D-0E-0F-10-11-12")
        );
        assert_eq!(
            decimal.format_buffer(FORMATTING_TEST_VALUES),
            String::from("10-11-12-13-14-15-16-17-18")
        );
        assert_eq!(
            octal.format_buffer(FORMATTING_TEST_VALUES),
            String::from("012-013-014-015-016-017-020-021-022")
        );
        assert_eq!(
            binary.format_buffer(FORMATTING_TEST_VALUES),
            String::from(
                "00001010-00001011-00001100-00001101-00001110-00001111-00010000-00010001-00010010"
            )
        );
    }

    #[test]
    fn test_owned_separator() {
        // Test new_owned with runtime strings
        let runtime_sep = String::from(" | ");
        let formatter = DecimalFormatter::new_owned(Some(runtime_sep));
        assert_eq!(
            formatter.format_buffer(FORMATTING_TEST_VALUES),
            String::from("10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18")
        );

        // Test with None (should use default)
        let formatter = OctalFormatter::new_owned(None);
        assert_eq!(formatter.get_separator(), ":");
    }

    #[test]
    fn test_from_cow() {
        // Test with borrowed Cow
        let sep_borrowed: Cow<'static, str> = Cow::Borrowed(" | ");
        let formatter = DecimalFormatter::from(sep_borrowed);
        assert_eq!(formatter.get_separator(), " | ");

        // Test with owned Cow
        let sep_owned: Cow<'static, str> = Cow::Owned(String::from(" | "));
        let formatter = OctalFormatter::from(sep_owned);
        assert_eq!(formatter.get_separator(), " | ");

        // Test using .into()
        let formatter: UppercaseHexadecimalFormatter = Cow::Borrowed("-").into();
        assert_eq!(
            formatter.format_buffer(FORMATTING_TEST_VALUES),
            String::from("0A-0B-0C-0D-0E-0F-10-11-12")
        );

        // Test all formatters
        let lowercase_hex: LowercaseHexadecimalFormatter = Cow::Borrowed(" ").into();
        let binary: BinaryFormatter = Cow::<'static, str>::Owned(String::from(",")).into();

        assert_eq!(lowercase_hex.get_separator(), " ");
        assert_eq!(binary.get_separator(), ",");
    }

    #[test]
    fn test_from_static_str() {
        // Test From<&str> with string literals
        let formatter = DecimalFormatter::from("-");
        assert_eq!(formatter.get_separator(), "-");
        assert_eq!(
            formatter.format_buffer(FORMATTING_TEST_VALUES),
            String::from("10-11-12-13-14-15-16-17-18")
        );

        // Test using .into()
        let formatter: OctalFormatter = " | ".into();
        assert_eq!(formatter.get_separator(), " | ");

        // Test with runtime &str (this is the key improvement)
        let runtime_string = String::from(" -> ");
        let runtime_str: &str = runtime_string.as_str();
        let formatter: UppercaseHexadecimalFormatter = runtime_str.into();
        assert_eq!(formatter.get_separator(), " -> ");
        assert_eq!(
            formatter.format_buffer(FORMATTING_TEST_VALUES),
            String::from("0A -> 0B -> 0C -> 0D -> 0E -> 0F -> 10 -> 11 -> 12")
        );

        // Test all formatter types
        let _decimal: DecimalFormatter = " ".into();
        let _octal: OctalFormatter = ",".into();
        let _uppercase_hex: UppercaseHexadecimalFormatter = "-".into();
        let _lowercase_hex: LowercaseHexadecimalFormatter = "::".into();
        let _binary: BinaryFormatter = "_".into();
    }

    #[test]
    fn test_from_string() {
        // Test From<String>
        let separator = String::from(" -> ");
        let formatter = DecimalFormatter::from(separator);
        assert_eq!(formatter.get_separator(), " -> ");
        assert_eq!(
            formatter.format_buffer(FORMATTING_TEST_VALUES),
            String::from("10 -> 11 -> 12 -> 13 -> 14 -> 15 -> 16 -> 17 -> 18")
        );

        // Test using .into()
        let formatter: UppercaseHexadecimalFormatter = String::from(" ").into();
        assert_eq!(
            formatter.format_buffer(FORMATTING_TEST_VALUES),
            String::from("0A 0B 0C 0D 0E 0F 10 11 12")
        );

        // Test all formatter types
        let _decimal: DecimalFormatter = String::from("-").into();
        let _octal: OctalFormatter = String::from(",").into();
        let _uppercase_hex: UppercaseHexadecimalFormatter = String::from("::").into();
        let _lowercase_hex: LowercaseHexadecimalFormatter = String::from(" | ").into();
        let _binary: BinaryFormatter = String::from("_").into();
    }

    #[test]
    fn test_from_option_static_str() {
        // Test From<Option<&str>> with Some
        let formatter = DecimalFormatter::from(Some("-"));
        assert_eq!(formatter.get_separator(), "-");

        // Test From<Option<&str>> with None (should use default)
        let formatter = OctalFormatter::from(None as Option<&str>);
        assert_eq!(formatter.get_separator(), ":");

        // Test using .into()
        let formatter: UppercaseHexadecimalFormatter = Some(" ").into();
        assert_eq!(formatter.get_separator(), " ");

        let formatter: LowercaseHexadecimalFormatter = (None as Option<&str>).into();
        assert_eq!(formatter.get_separator(), ":");

        // Test with runtime &str
        let runtime_string = String::from(" | ");
        let runtime_str: &str = runtime_string.as_str();
        let formatter: BinaryFormatter = Some(runtime_str).into();
        assert_eq!(formatter.get_separator(), " | ");

        // Test all formatter types
        let _decimal: DecimalFormatter = Some("->").into();
        let _octal: OctalFormatter = (None as Option<&str>).into();
        let _uppercase_hex: UppercaseHexadecimalFormatter = Some(",").into();
        let _lowercase_hex: LowercaseHexadecimalFormatter = Some("::").into();
        let _binary: BinaryFormatter = (None as Option<&str>).into();
    }

    #[test]
    fn test_from_option_string() {
        // Test From<Option<String>> with Some
        let separator = Some(String::from(" | "));
        let formatter = DecimalFormatter::from(separator);
        assert_eq!(formatter.get_separator(), " | ");

        // Test From<Option<String>> with None (should use default)
        let formatter = BinaryFormatter::from(None as Option<String>);
        assert_eq!(formatter.get_separator(), ":");

        // Test using .into()
        let formatter: UppercaseHexadecimalFormatter = Some(String::from("-")).into();
        assert_eq!(formatter.get_separator(), "-");

        let formatter: OctalFormatter = (None as Option<String>).into();
        assert_eq!(formatter.get_separator(), ":");

        // Test all formatter types
        let _decimal: DecimalFormatter = Some(String::from("::")).into();
        let _octal: OctalFormatter = (None as Option<String>).into();
        let _uppercase_hex: UppercaseHexadecimalFormatter = Some(String::from(" ")).into();
        let _lowercase_hex: LowercaseHexadecimalFormatter = Some(String::from(",")).into();
        let _binary: BinaryFormatter = (None as Option<String>).into();
    }

    #[test]
    fn test_format_byte() {
        // Test individual byte formatting
        let decimal = DecimalFormatter::new_default();
        let octal = OctalFormatter::new_default();
        let uppercase_hex = UppercaseHexadecimalFormatter::new_default();
        let lowercase_hex = LowercaseHexadecimalFormatter::new_default();
        let binary = BinaryFormatter::new_default();

        let byte = 255u8;
        assert_eq!(decimal.format_byte(&byte), "255");
        assert_eq!(octal.format_byte(&byte), "377");
        assert_eq!(uppercase_hex.format_byte(&byte), "FF");
        assert_eq!(lowercase_hex.format_byte(&byte), "ff");
        assert_eq!(binary.format_byte(&byte), "11111111");

        let byte = 0u8;
        assert_eq!(decimal.format_byte(&byte), "0");
        assert_eq!(octal.format_byte(&byte), "000");
        assert_eq!(uppercase_hex.format_byte(&byte), "00");
        assert_eq!(lowercase_hex.format_byte(&byte), "00");
        assert_eq!(binary.format_byte(&byte), "00000000");
    }

    #[test]
    fn test_edge_cases() {
        let formatter = DecimalFormatter::new_default();

        // Empty buffer
        assert_eq!(formatter.format_buffer(&[]), "");

        // Single byte
        assert_eq!(formatter.format_buffer(&[42]), "42");

        // Multi-character separator
        let formatter = DecimalFormatter::new(Some(" -> "));
        assert_eq!(formatter.format_buffer(&[1, 2, 3]), "1 -> 2 -> 3");

        // Empty string separator
        let formatter = DecimalFormatter::new(Some(""));
        assert_eq!(formatter.format_buffer(&[10, 11, 12]), "101112");
    }

    #[test]
    fn test_default_trait() {
        // Test Default implementation
        let decimal = DecimalFormatter::default();
        let octal = OctalFormatter::default();
        let uppercase_hex = UppercaseHexadecimalFormatter::default();
        let lowercase_hex = LowercaseHexadecimalFormatter::default();
        let binary = BinaryFormatter::default();

        // All should use default separator
        assert_eq!(decimal.get_separator(), ":");
        assert_eq!(octal.get_separator(), ":");
        assert_eq!(uppercase_hex.get_separator(), ":");
        assert_eq!(lowercase_hex.get_separator(), ":");
        assert_eq!(binary.get_separator(), ":");
    }

    #[test]
    fn test_clone() {
        // Test Clone trait
        let original = DecimalFormatter::new(Some("-"));
        let cloned = original.clone();

        assert_eq!(original.get_separator(), cloned.get_separator());
        assert_eq!(
            original.format_buffer(&[1, 2, 3]),
            cloned.format_buffer(&[1, 2, 3])
        );

        // Verify they're independent (though Cow makes this subtle)
        let original_result = original.format_buffer(&[10, 20]);
        let cloned_result = cloned.format_buffer(&[10, 20]);
        assert_eq!(original_result, cloned_result);
    }

    fn assert_unpin<T: Unpin>() {}

    #[test]
    fn test_unpin() {
        assert_unpin::<BinaryFormatter>();
        assert_unpin::<DecimalFormatter>();
        assert_unpin::<LowercaseHexadecimalFormatter>();
        assert_unpin::<UppercaseHexadecimalFormatter>();
        assert_unpin::<OctalFormatter>();
    }

    #[test]
    fn test_trait_object_safety() {
        // Assert trait object construction.
        let lowercase_hexadecimal: Box<dyn BufferFormatter> =
            Box::new(LowercaseHexadecimalFormatter::new(None));
        let uppercase_hexadecimal: Box<dyn BufferFormatter> =
            Box::new(UppercaseHexadecimalFormatter::new(None));
        let decimal: Box<dyn BufferFormatter> = Box::new(DecimalFormatter::new(None));
        let octal: Box<dyn BufferFormatter> = Box::new(OctalFormatter::new(None));
        let binary: Box<dyn BufferFormatter> = Box::new(BinaryFormatter::new(None));

        // Assert that trait object methods are dispatchable.
        assert_eq!(lowercase_hexadecimal.get_separator(), ":");
        assert!(!lowercase_hexadecimal.format_buffer(b"test").is_empty());

        assert_eq!(uppercase_hexadecimal.get_separator(), ":");
        assert!(!uppercase_hexadecimal.format_buffer(b"test").is_empty());

        assert_eq!(decimal.get_separator(), ":");
        assert!(!decimal.format_buffer(b"test").is_empty());

        assert_eq!(octal.get_separator(), ":");
        assert!(!octal.format_buffer(b"test").is_empty());

        assert_eq!(binary.get_separator(), ":");
        assert!(!binary.format_buffer(b"test").is_empty());
    }

    fn assert_buffer_formatter<T: BufferFormatter>() {}

    #[test]
    fn test_box() {
        assert_buffer_formatter::<Box<dyn BufferFormatter>>();
        assert_buffer_formatter::<Box<LowercaseHexadecimalFormatter>>();
        assert_buffer_formatter::<Box<UppercaseHexadecimalFormatter>>();
        assert_buffer_formatter::<Box<DecimalFormatter>>();
        assert_buffer_formatter::<Box<OctalFormatter>>();
        assert_buffer_formatter::<Box<BinaryFormatter>>();
    }

    fn assert_send<T: Send>() {}

    #[test]
    fn test_send() {
        assert_send::<LowercaseHexadecimalFormatter>();
        assert_send::<UppercaseHexadecimalFormatter>();
        assert_send::<DecimalFormatter>();
        assert_send::<OctalFormatter>();
        assert_send::<BinaryFormatter>();

        assert_send::<Box<dyn BufferFormatter>>();
        assert_send::<Box<LowercaseHexadecimalFormatter>>();
        assert_send::<Box<UppercaseHexadecimalFormatter>>();
        assert_send::<Box<DecimalFormatter>>();
        assert_send::<Box<OctalFormatter>>();
        assert_send::<Box<BinaryFormatter>>();

        // Arc wrapper types
        assert_send::<std::sync::Arc<LowercaseHexadecimalFormatter>>();
        assert_send::<std::sync::Arc<UppercaseHexadecimalFormatter>>();
        assert_send::<std::sync::Arc<DecimalFormatter>>();
        assert_send::<std::sync::Arc<OctalFormatter>>();
        assert_send::<std::sync::Arc<BinaryFormatter>>();
        assert_send::<std::sync::Arc<dyn BufferFormatter + Sync>>();
    }

    #[test]
    fn test_reference_impl() {
        // Test &'static T implementation
        static FORMATTER: DecimalFormatter = DecimalFormatter {
            separator: std::borrow::Cow::Borrowed("-"),
        };

        let formatter_ref: &'static DecimalFormatter = &FORMATTER;
        assert_eq!(formatter_ref.get_separator(), "-");
        assert_eq!(
            formatter_ref.format_buffer(FORMATTING_TEST_VALUES),
            String::from("10-11-12-13-14-15-16-17-18")
        );

        // Test that reference works as BufferFormatter
        fn takes_formatter(f: &impl BufferFormatter, data: &[u8]) -> String {
            f.format_buffer(data)
        }

        let formatter = OctalFormatter::new_default();
        let result = takes_formatter(&formatter, &[10, 11, 12]);
        assert_eq!(result, "012:013:014");

        // Test with Arc-wrapped formatter
        let arc_formatter = Arc::new(UppercaseHexadecimalFormatter::new(Some(" ")));
        let result = takes_formatter(&*arc_formatter, &[0xCA, 0xFE]);
        assert_eq!(result, "CA FE");
    }

    #[test]
    fn test_arc_impl() {
        // Test Arc<T> implementation
        let formatter = Arc::new(DecimalFormatter::new(Some(" -> ")));
        assert_eq!(formatter.get_separator(), " -> ");
        assert_eq!(
            formatter.format_buffer(FORMATTING_TEST_VALUES),
            String::from("10 -> 11 -> 12 -> 13 -> 14 -> 15 -> 16 -> 17 -> 18")
        );

        // Test Arc can be cloned and shared
        let formatter_clone = Arc::clone(&formatter);
        assert_eq!(formatter_clone.format_buffer(&[42, 43]), "42 -> 43");

        // Test all formatter types with Arc
        let octal = Arc::new(OctalFormatter::new_default());
        assert_eq!(octal.format_buffer(&[8, 9, 10]), "010:011:012");

        let uppercase_hex = Arc::new(UppercaseHexadecimalFormatter::new(Some(",")));
        assert_eq!(uppercase_hex.format_buffer(&[255, 254]), "FF,FE");

        let lowercase_hex = Arc::new(LowercaseHexadecimalFormatter::new(Some(" ")));
        assert_eq!(lowercase_hex.format_buffer(&[0xAB, 0xCD]), "ab cd");

        let binary = Arc::new(BinaryFormatter::new(Some("|")));
        assert_eq!(binary.format_buffer(&[3, 7]), "00000011|00000111");
    }

    #[test]
    fn test_arc_trait_object() {
        // Test Arc<dyn BufferFormatter + Sync>
        let formatter: Arc<dyn BufferFormatter + Sync> = Arc::new(DecimalFormatter::new(Some("-")));

        assert_eq!(formatter.get_separator(), "-");
        assert_eq!(formatter.format_buffer(&[1, 2, 3]), "1-2-3");

        // Test with different formatter types
        let formatters: Vec<Arc<dyn BufferFormatter + Sync>> = vec![
            Arc::new(DecimalFormatter::new_default()),
            Arc::new(OctalFormatter::new_default()),
            Arc::new(UppercaseHexadecimalFormatter::new_default()),
            Arc::new(LowercaseHexadecimalFormatter::new_default()),
            Arc::new(BinaryFormatter::new_default()),
        ];

        let data = vec![10u8];
        let results: Vec<String> = formatters.iter().map(|f| f.format_buffer(&data)).collect();

        assert_eq!(results[0], "10"); // Decimal
        assert_eq!(results[1], "012"); // Octal
        assert_eq!(results[2], "0A"); // Uppercase Hex
        assert_eq!(results[3], "0a"); // Lowercase Hex
        assert_eq!(results[4], "00001010"); // Binary

        // Test Arc trait object can be cloned
        let formatter_clone = Arc::clone(&formatters[0]);
        assert_eq!(formatter_clone.format_buffer(&[42]), "42");
    }

    #[test]
    fn test_arc_thread_safety() {
        // Test that Arc<T: BufferFormatter> can be shared across threads
        let formatter = Arc::new(DecimalFormatter::new(Some(" | ")));
        let formatter_clone = Arc::clone(&formatter);

        let handle = thread::spawn(move || formatter_clone.format_buffer(&[1, 2, 3, 4, 5]));

        let result = handle.join().unwrap();
        assert_eq!(result, "1 | 2 | 3 | 4 | 5");

        // Original formatter still works
        assert_eq!(formatter.format_buffer(&[10, 20]), "10 | 20");
    }

    #[test]
    fn test_wrapper_types_in_box() {
        // Test that Arc types work
        let arc_formatter = Arc::new(OctalFormatter::new(Some("-")));
        assert_eq!(arc_formatter.format_buffer(&[8, 9]), "010-011");

        // Test Arc trait object
        let arc_trait: Arc<dyn BufferFormatter + Sync> = Arc::new(DecimalFormatter::new(Some(" ")));
        assert_eq!(arc_trait.format_buffer(&[1, 2, 3]), "1 2 3");
    }

    #[test]
    fn test_partial_eq() {
        // Test equality with same separator
        let formatter1 = DecimalFormatter::new(Some("-"));
        let formatter2 = DecimalFormatter::new(Some("-"));
        assert_eq!(formatter1, formatter2);

        // Test equality with default separator
        let formatter3 = DecimalFormatter::new_default();
        let formatter4 = DecimalFormatter::new(None);
        assert_eq!(formatter3, formatter4);

        // Test inequality with different separators
        let formatter5 = DecimalFormatter::new(Some("-"));
        let formatter6 = DecimalFormatter::new(Some(":"));
        assert_ne!(formatter5, formatter6);

        // Note: different formatter types cannot be compared in Rust (type mismatch is a compile-time error);
        // this test only checks that same-type formatters with the same separator are equal.
        let octal1 = OctalFormatter::new(Some(" "));
        let octal2 = OctalFormatter::new(Some(" "));
        assert_eq!(octal1, octal2);

        // Test with static constructor
        let formatter7 = UppercaseHexadecimalFormatter::new_static(Some("-"));
        let formatter8 = UppercaseHexadecimalFormatter::new(Some("-"));
        assert_eq!(formatter7, formatter8);

        // Test with owned constructor
        let formatter9 = LowercaseHexadecimalFormatter::new_owned(Some(String::from(",")));
        let formatter10 = LowercaseHexadecimalFormatter::new(Some(","));
        assert_eq!(formatter9, formatter10);
    }

    #[test]
    fn test_hash() {
        use std::collections::HashMap;

        // Test that formatters with same separator hash to same value
        let formatter1 = DecimalFormatter::new(Some("-"));
        let formatter2 = DecimalFormatter::new(Some("-"));

        let mut map = HashMap::new();
        map.insert(formatter1, "first");
        map.insert(formatter2, "second"); // Should overwrite "first"

        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&DecimalFormatter::new(Some("-"))), Some(&"second"));

        // Test using formatters in HashSet
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(OctalFormatter::new(Some(":")));
        set.insert(OctalFormatter::new(Some(":")));
        set.insert(OctalFormatter::new(Some("-")));

        assert_eq!(set.len(), 2); // Two unique separators

        // Test that all formatter types can be hashed (separately)
        let mut decimal_set = HashSet::new();
        decimal_set.insert(DecimalFormatter::new_default());
        decimal_set.insert(DecimalFormatter::new(Some("-")));
        assert_eq!(decimal_set.len(), 2);

        let mut octal_set = HashSet::new();
        octal_set.insert(OctalFormatter::new_default());
        assert_eq!(octal_set.len(), 1);

        let mut upper_hex_set = HashSet::new();
        upper_hex_set.insert(UppercaseHexadecimalFormatter::new(Some(" ")));
        assert_eq!(upper_hex_set.len(), 1);

        let mut lower_hex_set = HashSet::new();
        lower_hex_set.insert(LowercaseHexadecimalFormatter::new(Some(" ")));
        assert_eq!(lower_hex_set.len(), 1);

        let mut binary_set = HashSet::new();
        binary_set.insert(BinaryFormatter::new_static(Some(",")));
        assert_eq!(binary_set.len(), 1);
    }

    #[test]
    fn test_display() {
        // Test Display implementation with regular separators
        let formatter = DecimalFormatter::new(Some("-"));
        assert_eq!(
            format!("{}", formatter),
            "DecimalFormatter(separator: \"-\")"
        );

        let formatter = OctalFormatter::new_default();
        assert_eq!(format!("{}", formatter), "OctalFormatter(separator: \":\")");

        let formatter = UppercaseHexadecimalFormatter::new(Some(" | "));
        assert_eq!(
            format!("{}", formatter),
            "UppercaseHexadecimalFormatter(separator: \" | \")"
        );

        let formatter = LowercaseHexadecimalFormatter::new(Some(""));
        assert_eq!(
            format!("{}", formatter),
            "LowercaseHexadecimalFormatter(separator: \"\")"
        );

        let formatter = BinaryFormatter::new_owned(Some(String::from(" -> ")));
        assert_eq!(
            format!("{}", formatter),
            "BinaryFormatter(separator: \" -> \")"
        );

        // Test that Display output is useful for logging
        let formatter = DecimalFormatter::new_static(Some("::"));
        let output = format!("Using formatter: {}", formatter);
        assert_eq!(
            output,
            "Using formatter: DecimalFormatter(separator: \"::\")"
        );
    }

    #[test]
    fn test_display_with_special_characters() {
        // Test Display implementation properly escapes special characters
        // This prevents log forging and ensures unambiguous output

        // Test newline character
        let formatter = DecimalFormatter::new(Some("\n"));
        assert_eq!(
            format!("{}", formatter),
            "DecimalFormatter(separator: \"\\n\")"
        );

        // Test tab character
        let formatter = OctalFormatter::new(Some("\t"));
        assert_eq!(
            format!("{}", formatter),
            "OctalFormatter(separator: \"\\t\")"
        );

        // Test carriage return
        let formatter = UppercaseHexadecimalFormatter::new(Some("\r"));
        assert_eq!(
            format!("{}", formatter),
            "UppercaseHexadecimalFormatter(separator: \"\\r\")"
        );

        // Test double quote character
        let formatter = LowercaseHexadecimalFormatter::new(Some("\""));
        assert_eq!(
            format!("{}", formatter),
            "LowercaseHexadecimalFormatter(separator: \"\\\"\")"
        );

        // Test backslash character
        let formatter = BinaryFormatter::new(Some("\\"));
        assert_eq!(
            format!("{}", formatter),
            "BinaryFormatter(separator: \"\\\\\")"
        );

        // Test multiple special characters combined
        let formatter = DecimalFormatter::new(Some("\n\t\r\"\\"));
        assert_eq!(
            format!("{}", formatter),
            "DecimalFormatter(separator: \"\\n\\t\\r\\\"\\\\\")"
        );

        // Test potential log forging attempt
        let malicious_sep = "\nINFO: Fake log entry";
        let formatter = OctalFormatter::new(Some(malicious_sep));
        let output = format!("{}", formatter);
        // The newline should be escaped, preventing it from creating a new log line
        assert!(output.contains("\\n"));
        assert!(!output.contains("\nINFO"));
        assert_eq!(
            output,
            "OctalFormatter(separator: \"\\nINFO: Fake log entry\")"
        );

        // Test unicode and other edge cases
        let formatter = UppercaseHexadecimalFormatter::new(Some("→\u{200B}←")); // Arrow with zero-width space
        let output = format!("{}", formatter);
        // Should contain the escaped or properly formatted unicode
        assert!(output.contains("separator:"));
    }
}
