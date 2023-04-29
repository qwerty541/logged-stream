use std::iter::Iterator;

const DEFAULT_SEPARATOR: &str = ":";

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// Trait
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

pub trait BufferFormatter: 'static {
    fn get_separator(&self) -> &'static str;

    fn format_byte(&self, byte: &u8) -> String;

    fn format_buffer(&self, buffer: &[u8]) -> String {
        buffer
            .iter()
            .map(|b| self.format_byte(b))
            .collect::<Vec<String>>()
            .join(self.get_separator())
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// DecimalFormatter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy)]
pub struct DecimalFormatter {
    separator: &'static str,
}

impl DecimalFormatter {
    pub fn new(separator: Option<&'static str>) -> Self {
        Self {
            separator: separator.unwrap_or(DEFAULT_SEPARATOR),
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

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// OctalFormatter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy)]
pub struct OctalFormatter {
    separator: &'static str,
}

impl OctalFormatter {
    pub fn new(separator: Option<&'static str>) -> Self {
        Self {
            separator: separator.unwrap_or(DEFAULT_SEPARATOR),
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

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// HexDecimalFormatter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy)]
pub struct HexDecimalFormatter {
    separator: &'static str,
}

impl HexDecimalFormatter {
    pub fn new(separator: Option<&'static str>) -> Self {
        Self {
            separator: separator.unwrap_or(DEFAULT_SEPARATOR),
        }
    }
}

impl BufferFormatter for HexDecimalFormatter {
    fn get_separator(&self) -> &'static str {
        self.separator
    }

    fn format_byte(&self, byte: &u8) -> String {
        format!("{:02x}", byte)
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// BInaryFormatter
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy)]
pub struct BinaryFormatter {
    separator: &'static str,
}

impl BinaryFormatter {
    pub fn new(separator: Option<&'static str>) -> Self {
        Self {
            separator: separator.unwrap_or(DEFAULT_SEPARATOR),
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

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// Tests
//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use crate::buffer_formatter::BinaryFormatter;
    use crate::buffer_formatter::BufferFormatter;
    use crate::buffer_formatter::DecimalFormatter;
    use crate::buffer_formatter::HexDecimalFormatter;
    use crate::buffer_formatter::OctalFormatter;
    use std::marker::Unpin;

    fn assert_unpin<T: Unpin>() {}

    #[test]
    fn test_unpin() {
        assert_unpin::<BinaryFormatter>();
        assert_unpin::<DecimalFormatter>();
        assert_unpin::<HexDecimalFormatter>();
        assert_unpin::<OctalFormatter>();
    }

    #[test]
    fn test_trait_object_safety() {
        // Assert traint object construct.
        let hexdecimal: Box<dyn BufferFormatter> = Box::new(HexDecimalFormatter::new(None));
        let decimal: Box<dyn BufferFormatter> = Box::new(DecimalFormatter::new(None));
        let octal: Box<dyn BufferFormatter> = Box::new(OctalFormatter::new(None));
        let binary: Box<dyn BufferFormatter> = Box::new(BinaryFormatter::new(None));

        // Assert that trait object methods are dispatchable.
        _ = hexdecimal.get_separator();
        _ = hexdecimal.format_buffer(b"qwertyuiop");

        _ = decimal.get_separator();
        _ = decimal.format_buffer(b"qwertyuiop");

        _ = octal.get_separator();
        _ = octal.format_buffer(b"qwertyuiop");

        _ = binary.get_separator();
        _ = binary.format_buffer(b"qwertyuiop");
    }
}
