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

/// A format which is able to deserialise `Read` and serialise `Write`
pub trait Format<Read, Write>: Send + Sync {
    /// The HTTP content-type related to this format
    fn content_type(&self) -> &'static str;

    /// Read a value from the given [Read]
    fn read(&self, reader: &[u8]) -> Result<Read, Box<dyn Error>>;
    /// write the given value to the given [Write]
    fn write(&self, value: Write) -> Result<Vec<u8>, Box<dyn Error>>;
}

impl<Read, Write> dyn Format<Read, Write> {}
