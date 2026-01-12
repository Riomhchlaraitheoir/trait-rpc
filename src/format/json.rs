//! Provides support for the CBOR ([JavaScript Object Notation](https://www.json.org/)) format

use crate::format::{Format, IsFormat};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::error::Error;

const CONTENT_TYPE: &str = "application/json";

#[derive(Debug, Copy, Clone)]
/// [JavaScript Object Notation](https://www.json.org/)
pub struct Json;

impl IsFormat for Json {
    fn content_type(&self) -> &'static str {
        CONTENT_TYPE
    }
}

impl<Read, Write> Format<Read, Write> for Json
where
    Read: DeserializeOwned,
    Write: Serialize,
{
    fn read(&self, reader: &[u8]) -> Result<Read, Box<dyn Error + Send>> {
        serde_json::from_slice(reader).map_err(|error| Box::new(error) as Box<dyn Error + Send>)
    }

    fn write(&self, value: Write) -> Result<Vec<u8>, Box<dyn Error + Send>> {
        serde_json::to_vec(&value).map_err(|error| Box::new(error) as Box<dyn Error + Send>)
    }
}
