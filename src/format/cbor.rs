//! Provides support for the CBOR ([Concise Binary Object Representation](https://cbor.io/)) format

use crate::format::Format;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::error::Error;

const CONTENT_TYPE: &str = "application/cbor";

#[derive(Debug, Copy, Clone)]
/// [Concise Binary Object Representation](https://cbor.io/)
pub struct Cbor;

impl<Read, Write> Format<Read, Write> for Cbor
where Read: DeserializeOwned, Write: Serialize
{
    fn content_type(&self) -> &'static str {
        CONTENT_TYPE
    }

    fn read(&self, reader: &[u8]) -> Result<Read, Box<dyn Error>> {
        ciborium::from_reader(reader).map_err(|error| Box::new(error) as Box<dyn Error>)
    }

    fn write(&self, value: Write) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut buffer = Vec::new();
        ciborium::into_writer(&value, &mut buffer).map_err(|error| Box::new(error) as Box<dyn Error>)?;
        Ok(buffer)
    }
}
