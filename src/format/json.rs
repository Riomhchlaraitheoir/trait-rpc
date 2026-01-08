//! Provides support for the CBOR ([JavaScript Object Notation](https://www.json.org/)) format

use std::error::Error;
use serde::de::DeserializeOwned;
use serde::Serialize;
use crate::format::{Format};

const CONTENT_TYPE: & str = "application/json";

#[derive(Debug, Copy, Clone)]
/// [JavaScript Object Notation](https://www.json.org/)
pub struct Json;

impl<Read, Write> Format<Read, Write> for Json
where Read: DeserializeOwned, Write: Serialize
{
    fn content_type(&self) -> &'static str {
        CONTENT_TYPE
    }

    fn read(&self, reader: &[u8]) -> Result<Read, Box<dyn Error>> {
        serde_json::from_slice(reader).map_err(|error| Box::new(error) as Box<dyn Error>)
    }

    fn write(&self, value: Write) -> Result<Vec<u8>, Box<dyn Error>> {
        serde_json::to_vec(&value).map_err(|error| Box::new(error) as Box<dyn Error>)
    }
}
