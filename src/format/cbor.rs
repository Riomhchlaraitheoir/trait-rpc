use crate::format::{Format, FormatInfo};
use std::io;
use serde::de::DeserializeOwned;
use serde::Serialize;

const CONTENT_TYPE: &str = "application/cbor";

#[derive(Debug, Copy, Clone)]
/// [Concise Binary Object Representation](https://cbor.io/)
pub struct Cbor;

impl<Read, Write> Format<Read, Write> for Cbor
where Read: DeserializeOwned, Write: Serialize
{
    const INFO: &'static FormatInfo = &FormatInfo {
        http_content_type: CONTENT_TYPE,
    };
    type ReadError = ciborium::de::Error<io::Error>;
    type WriteError = ciborium::ser::Error<io::Error>;

    fn read(&self, reader: impl io::Read) -> Result<Read, Self::ReadError> {
        ciborium::from_reader(reader)
    }

    fn write(&self, value: Write, writer: impl io::Write) -> Result<(), Self::WriteError> {
        ciborium::into_writer(&value, writer)
    }
}
