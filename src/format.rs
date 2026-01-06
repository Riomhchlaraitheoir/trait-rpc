//! Defines format behaviour

use std::error::Error;
use std::io;

#[cfg(feature = "json")]
mod json;
#[cfg(feature = "json")]
pub use json::Json;
#[cfg(feature = "cbor")]
mod cbor;
#[cfg(feature = "cbor")]
pub use cbor::Cbor;

/// A format which is able to deserialise `Read` and serialise `Write`
pub trait Format<Read, Write>: Send + Sync {
    /// INFO related to this format
    const INFO: &'static FormatInfo;

    /// An error which may occur during deserialisation
    type ReadError: Error + 'static;
    /// An error which may occur during serialisation
    type WriteError: Error + 'static;
    /// Read a value from the given [Read]
    fn read(&self, reader: impl io::Read) -> Result<Read, Self::ReadError>;
    /// write the given value to the given [Write]
    fn write(&self, value: Write, writer: impl io::Write) -> Result<(), Self::WriteError>;
}

/// A format which is able to deserialise `Read` and serialise `Write`
pub trait DynFormat<Readable, Writeable>: Send + Sync {
    /// Read a value from the given [Read]
    fn read(&self, reader: &mut dyn io::Read) -> Result<Readable, String>;
    /// write the given value to the given [Write]
    fn write(&self, value: Writeable, writer: &mut dyn io::Write) -> Result<(), String>;
    /// Returns info related to this format
    fn info(&self) -> &'static FormatInfo;
}

impl<F, Read, Write> DynFormat<Read, Write> for F where F: Format<Read, Write> {
    fn read(&self, reader: &mut dyn io::Read) -> Result<Read, String> {
        Format::<Read, Write>::read(self, reader).map_err(|err| err.to_string())
    }

    fn write(&self, value: Write, writer: &mut dyn io::Write) -> Result<(), String> {
        Format::<Read, Write>::write(self, value, writer).map_err(|err| err.to_string())
    }

    fn info(&self) -> &'static FormatInfo {
        F::INFO
    }
}

impl<Read, Write> dyn DynFormat<Read, Write> {}

/// Information on a particular format
pub struct FormatInfo {
    /// The MIME type for this format
    pub http_content_type: &'static str,
}


