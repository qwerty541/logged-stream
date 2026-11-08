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
/// [`LoggedStream`] calls [`format_buffer`] lazily, at most once per read or write, and only for the
/// reads and writes that a filter asks to see formatted or that are logged. Do not rely on it running
/// for every buffer; if your formatter has side effects that must, have the filter's
/// [`EventFilter::check`] call [`Event::record`] before it decides.
///
/// # Wrapping in an `Arc`
///
/// `BufferFormatter` is implemented for `Box<dyn BufferFormatter>` unconditionally, but the blanket
/// impl for `Arc<T>` additionally requires `T: Sync` — because `BufferFormatter` requires `Send`,
/// and `Arc<T>` is `Send` only when `T: Send + Sync`. The concrete formatters in this crate are
/// `Sync`, so `Arc<LowercaseHexadecimalFormatter>` and friends work directly. For an erased trait
/// object you must spell the bound out as `Arc<dyn BufferFormatter + Sync>`; a plain
/// `Arc<dyn BufferFormatter>` does **not** implement `BufferFormatter`.
///
/// [`format_buffer`]: BufferFormatter::format_buffer
/// [`LoggedStream`]: crate::LoggedStream
/// [`EventFilter::check`]: crate::EventFilter::check
/// [`Event::record`]: crate::Event::record
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

    // Forwarded too, so that a formatter overriding it keeps its override once boxed.
    #[inline]
    fn format_buffer(&self, buffer: &[u8]) -> String {
        (**self).format_buffer(buffer)
    }
}

impl<T: BufferFormatter + ?Sized + Sync> BufferFormatter for Arc<T> {
    #[inline]
    fn get_separator(&self) -> &str {
        (**self).get_separator()
    }

    #[inline]
    fn format_byte(&self, byte: &u8) -> String {
        (**self).format_byte(byte)
    }

    // Forwarded too, so that a formatter overriding it keeps its override in an `Arc`.
    #[inline]
    fn format_buffer(&self, buffer: &[u8]) -> String {
        (**self).format_buffer(buffer)
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
