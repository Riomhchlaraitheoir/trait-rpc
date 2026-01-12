//! Defines format behaviour
#![allow(clippy::missing_errors_doc, reason = "Errors are obvious")]

use std::error::Error;

#[cfg(feature = "json")]
pub mod json;
#[cfg(all(feature = "browser-json", target_arch = "wasm32"))]
pub mod browser_json;
#[cfg(all(feature = "browser-json", not(target_arch = "wasm32")))]
compile_error!("browser-json is only available on wasm32 arch");
#[cfg(feature = "cbor")]
pub mod cbor;

/// This specifies types which represent formats, but does not prescribe supporting any particular type
pub trait IsFormat {
    /// The HTTP content-type related to this format
    fn content_type(&self) -> &'static str;
}

/// A format which is able to deserialise `Read` and serialise `Write`
pub trait Format<Read, Write>: IsFormat + Send + Sync {
    /// Read a value from the given [Read]
    fn read(&self, reader: &[u8]) -> Result<Read, Box<dyn Error + Send>>;
    /// write the given value to the given [Write]
    fn write(&self, value: Write) -> Result<Vec<u8>, Box<dyn Error + Send>>;
}

impl<Read, Write> dyn Format<Read, Write> {}
