pub use serde;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::error::Error;
use thiserror::Error;

pub mod server;
#[cfg(feature = "reqwest")]
pub mod reqwest;

pub use macros::trait_link;

pub trait Transport {
    type Error: std::error::Error;
    fn send<Req, Resp>(&self, request: Req) -> impl Future<Output = Result<Resp, <Self as Transport>::Error>>
    where
        Req: Serialize,
        Resp: DeserializeOwned;
}

pub trait Rpc: Sync {
    type Request: Serialize + DeserializeOwned + Send;
    type Response: Serialize + DeserializeOwned;

    fn process(&self, request: Self::Request) -> impl Future<Output = Self::Response> + Send;
}

#[derive(Debug, Error, Clone)]
pub enum LinkError<T: Error> {
    #[error("Failed to send request: {0}")]
    Transport(#[from] T),
    /// Response was the wrong type, sent a request for one function, but received the response of a different one
    ///
    /// This is not an expected case and is simply included as an alternative to panicking in this case
    /// This error either means the server side is misbehaving quite badly, or the transport is not configured to the correct endpoint
    #[error("Response was the wrong type, sent a request for one function, but received the response of a different one")]
    WrongResponseType,
}
