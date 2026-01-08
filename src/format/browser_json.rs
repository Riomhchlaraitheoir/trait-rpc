//! Provides support for the CBOR ([JavaScript Object Notation](https://www.json.org/)) format, driven by the WASM API

use std::io;
use std::error::Error as StdError;
use std::string::FromUtf8Error;
use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;
use wasm_bindgen_futures::wasm_bindgen::JsValue;
use web_sys::js_sys;
use crate::format::{Format};

const CONTENT_TYPE: & str = "application/json";

#[derive(Debug, Copy, Clone)]
/// [JavaScript Object Notation](https://www.json.org/)
pub struct BrowserJson;

impl<Read, Write> Format<Read, Write> for BrowserJson
where Read: DeserializeOwned, Write: Serialize
{
    fn content_type(&self) -> &'static str {
        CONTENT_TYPE
    }

    fn read(&self, json: &[u8]) -> Result<Read, Box<dyn StdError>> {
        let json = String::from_utf8(Vec::from(json))?;
        let value = js_sys::JSON::parse(&json).map_err(Error::Parse)?;
        Ok(serde_wasm_bindgen::from_value(value)?)
    }

    fn write(&self, value: Write) -> Result<Vec<u8>, Box<dyn StdError>> {
        let value = serde_wasm_bindgen::to_value(&value)?;
        let json = js_sys::JSON::stringify(&value).map_err(Error::Parse)?;
        Ok(json.as_string().ok_or(Error::JsString)?.as_bytes().to_vec())
    }
}

/// An error which can occur while serialising/deserialising JSON
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to parse JSON string
    #[error("failed to parse json: {0:?}")]
    Parse(JsValue),
    /// Failed to translate between rust and JS objects
    #[error("failed to serialize json: {0}")]
    Serde(#[from] serde_wasm_bindgen::Error),
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
