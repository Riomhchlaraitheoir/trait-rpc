//! Provides support for the CBOR ([JavaScript Object Notation](https://www.json.org/)) format, driven by the WASM API

use std::io;
use std::error::Error as StdError;
use std::string::FromUtf8Error;
use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;
use web_sys::js_sys;
use crate::format::{Format, IsFormat};

const CONTENT_TYPE: & str = "application/json";

#[derive(Debug, Copy, Clone)]
/// [JavaScript Object Notation](https://www.json.org/)
pub struct BrowserJson;

impl IsFormat for BrowserJson {
    fn content_type(&self) -> &'static str {
        CONTENT_TYPE
    }
}

impl<Read, Write> Format<Read, Write> for BrowserJson
where Read: DeserializeOwned, Write: Serialize
{
    fn read(&self, json: &[u8]) -> Result<Read, Box<dyn StdError + Send>> {
        Self::read_impl(json).map_err(|e| Box::new(e) as Box<dyn StdError + Send>)
    }

    fn write(&self, value: Write) -> Result<Vec<u8>, Box<dyn StdError + Send>> {
        Self::write_impl(value).map_err(|e| Box::new(e) as Box<dyn StdError + Send>)
    }
}

impl BrowserJson {
    fn read_impl<T: DeserializeOwned>(json: &[u8]) -> Result<T, Error> {
        let json = String::from_utf8(Vec::from(json)).map_err(Error::Decode)?;
        let value = js_sys::JSON::parse(&json).map_err(|error| Error::Parse(format!("{error:?}")))?;
        serde_wasm_bindgen::from_value(value).map_err(|error| Error::Serde(error.to_string()))
    }

    fn write_impl(value: impl Serialize) -> Result<Vec<u8>, Error> {
        let value = serde_wasm_bindgen::to_value(&value).map_err(|error| Error::Serde(error.to_string()))?;
        let json = js_sys::JSON::stringify(&value).map_err(|error| Error::Parse(format!("{error:?}")))?;
        Ok(json.as_string().ok_or(Error::JsString)?.as_bytes().to_vec())
    }
}

/// An error which can occur while serialising/deserialising JSON
#[derive(Debug, Error)]
pub enum Error where Self: Send {
    /// Failed to parse JSON string
    #[error("failed to parse json: {0:?}")]
    Parse(String),
    /// Failed to translate between rust and JS objects
    #[error("failed to serialize json: {0}")]
    Serde(String),
    /// Failed io on read/write
    #[error(transparent)]
    IO(#[from] io::Error),
    /// Failed to read a bytes as string
    #[error("failed to decode string: {0}")]
    Decode(#[from] FromUtf8Error),
    /// Failed to read JS string as a rust string
    #[error("failed to translate string")]
    JsString
}
