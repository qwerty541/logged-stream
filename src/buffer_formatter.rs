use std::iter::Iterator;
use std::marker::Send;

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
    fn get_separator(&self) -> &'static str;

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
    fn get_separator(&self) -> &'static str {
        (**self).get_separator()
    }

    fn format_byte(&self, byte: &u8) -> String {
        (**self).format_byte(byte)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// DecimalFormatter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// This implementation of [`BufferFormatter`] trait formats provided bytes buffer in decimal number system.
#[derive(Debug, Clone, Copy)]
pub struct DecimalFormatter {
    separator: &'static str,
}

impl DecimalFormatter {
    /// Construct a new instance of [`DecimalFormatter`] using provided separator. In case if provided separator
    /// will be [`None`], than default separator (`:`) will be used.
    pub fn new(provided_separator: Option<&'static str>) -> Self {
        Self {
            separator: provided_separator.unwrap_or(DEFAULT_SEPARATOR),
        }
    }
}

impl BufferFormatter for DecimalFormatter {
    fn get_separator(&self) -> &'static str {
        self.separator
    }

    fn format_byte(&self, byte: &u8) -> String {
        format!("{}", byte)
    }
}

impl BufferFormatter for Box<DecimalFormatter> {
    fn get_separator(&self) -> &'static str {
        (**self).get_separator()
    }

    fn format_byte(&self, byte: &u8) -> String {
        (**self).format_byte(byte)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// OctalFormatter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// This implementation of [`BufferFormatter`] trait formats provided bytes buffer in octal number system.
#[derive(Debug, Clone, Copy)]
pub struct OctalFormatter {
    separator: &'static str,
}

impl OctalFormatter {
    /// Construct a new instance of [`OctalFormatter`] using provided separator. In case if provided separator
    /// will be [`None`], than default separator (`:`) will be used.
    pub fn new(provided_separator: Option<&'static str>) -> Self {
        Self {
            separator: provided_separator.unwrap_or(DEFAULT_SEPARATOR),
        }
    }
}

impl BufferFormatter for OctalFormatter {
    fn get_separator(&self) -> &'static str {
        self.separator
    }

    fn format_byte(&self, byte: &u8) -> String {
        format!("{:03o}", byte)
    }
}

impl BufferFormatter for Box<OctalFormatter> {
    fn get_separator(&self) -> &'static str {
        (**self).get_separator()
    }

    fn format_byte(&self, byte: &u8) -> String {
        (**self).format_byte(byte)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// UppercaseHexadecimalFormatter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// This implementation of [`BufferFormatter`] trait formats provided bytes buffer in hexadecimal number system.
#[derive(Debug, Clone, Copy)]
pub struct UppercaseHexadecimalFormatter {
    separator: &'static str,
}

impl UppercaseHexadecimalFormatter {
    /// Construct a new instance of [`UppercaseHexadecimalFormatter`] using provided separator. In case if provided separator
    /// will be [`None`], than default separator (`:`) will be used.
    pub fn new(provided_separator: Option<&'static str>) -> Self {
        Self {
            separator: provided_separator.unwrap_or(DEFAULT_SEPARATOR),
        }
    }
}

impl BufferFormatter for UppercaseHexadecimalFormatter {
    fn get_separator(&self) -> &'static str {
        self.separator
    }

    fn format_byte(&self, byte: &u8) -> String {
        format!("{:02X}", byte)
    }
}

impl BufferFormatter for Box<UppercaseHexadecimalFormatter> {
    fn get_separator(&self) -> &'static str {
        (**self).get_separator()
    }

    fn format_byte(&self, byte: &u8) -> String {
        (**self).format_byte(byte)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// LowercaseHexadecimalFormatter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// This implementation of [`BufferFormatter`] trait formats provided bytes buffer in hexdecimal number system.
#[derive(Debug, Clone, Copy)]
pub struct LowercaseHexadecimalFormatter {
    separator: &'static str,
}

impl LowercaseHexadecimalFormatter {
    /// Construct a new instance of [`LowercaseHexadecimalFormatter`] using provided separator. In case if provided separator
    /// will be [`None`], than default separator (`:`) will be used.
    pub fn new(provided_separator: Option<&'static str>) -> Self {
        Self {
            separator: provided_separator.unwrap_or(DEFAULT_SEPARATOR),
        }
    }
}

impl BufferFormatter for LowercaseHexadecimalFormatter {
    fn get_separator(&self) -> &'static str {
        self.separator
    }

    fn format_byte(&self, byte: &u8) -> String {
        format!("{:02x}", byte)
    }
}

impl BufferFormatter for Box<LowercaseHexadecimalFormatter> {
    fn get_separator(&self) -> &'static str {
        (**self).get_separator()
    }

    fn format_byte(&self, byte: &u8) -> String {
        (**self).format_byte(byte)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// BinaryFormatter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// This implementation of [`BufferFormatter`] trait formats provided bytes buffer in binary number system.
#[derive(Debug, Clone, Copy)]
pub struct BinaryFormatter {
    separator: &'static str,
}

impl BinaryFormatter {
    /// Construct a new instance of [`BinaryFormatter`] using provided separator. In case if provided separator
    /// will be [`None`], than default separator (`:`) will be used.
    pub fn new(provided_separator: Option<&'static str>) -> Self {
        Self {
            separator: provided_separator.unwrap_or(DEFAULT_SEPARATOR),
        }
    }
}

impl BufferFormatter for BinaryFormatter {
    fn get_separator(&self) -> &'static str {
        self.separator
    }

    fn format_byte(&self, byte: &u8) -> String {
        format!("{:08b}", byte)
    }
}

impl BufferFormatter for Box<BinaryFormatter> {
    fn get_separator(&self) -> &'static str {
        (**self).get_separator()
    }

    fn format_byte(&self, byte: &u8) -> String {
        (**self).format_byte(byte)
    }
}

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
    use std::convert::From;
    use std::marker::Send;
    use std::marker::Unpin;

    #[test]
    fn test_buffer_formatting() {
        let lowercase_hexadecimal = LowercaseHexadecimalFormatter::new(None);
        let uppercase_hexadecimal = UppercaseHexadecimalFormatter::new(None);
        let decimal = DecimalFormatter::new(None);
        let octal = OctalFormatter::new(None);
        let binary = BinaryFormatter::new(None);

        const TEST_VALUES: &[u8] = &[10, 11, 12, 13, 14, 15, 16, 17, 18];
        assert_eq!(
            lowercase_hexadecimal.format_buffer(TEST_VALUES),
            String::from("0a:0b:0c:0d:0e:0f:10:11:12")
        );
        assert_eq!(
            uppercase_hexadecimal.format_buffer(TEST_VALUES),
            String::from("0A:0B:0C:0D:0E:0F:10:11:12")
        );
        assert_eq!(
            decimal.format_buffer(TEST_VALUES),
            String::from("10:11:12:13:14:15:16:17:18")
        );
        assert_eq!(
            octal.format_buffer(TEST_VALUES),
            String::from("012:013:014:015:016:017:020:021:022")
        );
        assert_eq!(
            binary.format_buffer(TEST_VALUES),
            String::from(
                "00001010:00001011:00001100:00001101:00001110:00001111:00010000:00010001:00010010"
            )
        );
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
        // Assert traint object construct.
        let lowercase_hexadecimal: Box<dyn BufferFormatter> =
            Box::new(LowercaseHexadecimalFormatter::new(None));
        let uppercase_hexadecimal: Box<dyn BufferFormatter> =
            Box::new(UppercaseHexadecimalFormatter::new(None));
        let decimal: Box<dyn BufferFormatter> = Box::new(DecimalFormatter::new(None));
        let octal: Box<dyn BufferFormatter> = Box::new(OctalFormatter::new(None));
        let binary: Box<dyn BufferFormatter> = Box::new(BinaryFormatter::new(None));

        // Assert that trait object methods are dispatchable.
        _ = lowercase_hexadecimal.get_separator();
        _ = lowercase_hexadecimal.format_buffer(b"qwertyuiop");

        _ = uppercase_hexadecimal.get_separator();
        _ = uppercase_hexadecimal.format_buffer(b"qwertyuiop");

        _ = decimal.get_separator();
        _ = decimal.format_buffer(b"qwertyuiop");

        _ = octal.get_separator();
        _ = octal.format_buffer(b"qwertyuiop");

        _ = binary.get_separator();
        _ = binary.format_buffer(b"qwertyuiop");
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
