//! Provides support for the CBOR ([Concise Binary Object Representation](https://cbor.io/)) format

use crate::format::{Format, IsFormat};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::error::Error;

const CONTENT_TYPE: &str = "application/cbor";

#[derive(Debug, Copy, Clone)]
/// [Concise Binary Object Representation](https://cbor.io/)
pub struct Cbor;

impl IsFormat for Cbor {
    fn content_type(&self) -> &'static str {
        CONTENT_TYPE
    }
}

impl<Read, Write> Format<Read, Write> for Cbor
where Read: DeserializeOwned, Write: Serialize
{
    fn read(&self, reader: &[u8]) -> Result<Read, Box<dyn Error + Send>> {
        ciborium::from_reader(reader).map_err(|error| Box::new(error) as Box<dyn Error + Send>)
    }

    fn write(&self, value: Write) -> Result<Vec<u8>, Box<dyn Error + Send>> {
        let mut buffer = Vec::new();
        ciborium::into_writer(&value, &mut buffer).map_err(|error| Box::new(error) as Box<dyn Error + Send>)?;
        Ok(buffer)
    }
}
