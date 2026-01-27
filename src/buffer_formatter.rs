use std::borrow::Cow;

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
        #[derive(Debug, Clone)]
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
        use std::borrow::Cow;

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
    }
}
