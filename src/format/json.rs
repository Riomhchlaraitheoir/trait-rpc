use serde::de::DeserializeOwned;
use serde::Serialize;
use crate::format::{Format, FormatInfo};

const CONTENT_TYPE: & str = "application/json";

#[derive(Debug, Copy, Clone)]
/// [JavaScript Object Notation](https://www.json.org/)
pub struct Json;

impl<Read, Write> Format<Read, Write> for Json
where Read: DeserializeOwned, Write: Serialize
{
    const INFO: &'static FormatInfo = &FormatInfo {
        http_content_type: CONTENT_TYPE,
    };
    type ReadError = serde_json::Error;
    type WriteError = serde_json::Error;

    fn read(&self, reader: impl std::io::Read) -> Result<Read, Self::ReadError> {
        serde_json::from_reader(reader)
    }

    fn write(&self, value: Write, writer: impl std::io::Write) -> Result<(), Self::WriteError> {
        serde_json::to_writer(writer, &value)
    }
}
