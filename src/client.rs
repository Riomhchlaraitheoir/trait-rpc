//! Contains modules for individual client implementations
#![allow(clippy::future_not_send, reason = "Cannot explicitly make futures `Send` while supporting WASM")]

use crate::format::Format;
use bon::bon;
use std::error::Error;
use std::fmt::Debug;
use thiserror::Error;

/// Implementation for making requests from browser wasm using the Fetch API
#[cfg(all(feature = "browser", target_arch = "wasm32"))]
pub mod browser;
#[cfg(all(feature = "browser", not(target_arch = "wasm32")))]
compile_error!("browser feature is only available for wasm32 target arch");
/// Implementation for making requests using the reqwest crate
#[cfg(feature = "reqwest")]
pub mod reqwest;
/// Implementation for making requests using the reqwest crate
#[cfg(all(feature = "reqwest-blocking", not(target_arch = "wasm32")))]
pub mod reqwest_blocking;
#[cfg(all(feature = "reqwest-blocking", target_arch = "wasm32"))]
compile_error!("reqwest-blocking feature is not available for wasm32 target arch");

#[must_use]
/// Return a client builder
pub const fn builder() -> Builder {
    Builder
}

/// A client builder for building any kind of client
pub struct Builder;

#[bon]
#[allow(clippy::unused_self)]
impl Builder {
    /// Build an asynchronous client
    #[builder(finish_fn = build)]
    pub const fn non_blocking<F, T>(
        self,
        /// The format to be used for serialisation and deserialisation
        ///
        /// IMPORTANT, the format must be supported by the server
        format: F,
        /// The transport mechanism to use
        ///
        /// Available options are:
        ///  * [request](reqwest::Reqwest)
        ///  * [browser](browser::Browser) (WASM-only)
        transport: T
    ) -> SimpleClient<F, T>
    where T: AsyncTransport
    {
        SimpleClient { format, transport }
    }

    /// Build a blocking client
    #[builder(finish_fn = build)]
    pub const fn blocking<F, T>(
        self,
        /// The format to be used for serialisation and deserialisation
        ///
        /// IMPORTANT, the format must be supported by the server
        format: F,
        /// The transport mechanism to use
        ///
        /// Available options are:
        ///  * [request](reqwest::Reqwest)
        ///  * [browser](browser::Browser) (WASM-only)
        transport: T
    ) -> SimpleClient<F, T>
    where T: BlockingTransport
    {
        SimpleClient { format, transport }
    }
}

/// A client implementation for sending requests asynchronously
pub trait AsyncClient<Req, Resp>: Clone {
    /// The error that can happen during send
    type Error: Error + MaybeWrongResponse + From<WrongResponseType> + 'static;
    /// Send a request and receive a response
    fn send(&self, request: Req) -> impl Future<Output = Result<Resp, Self::Error>>;
}

/// A client implementation for sending requests in a blocking manner
pub trait BlockingClient<Req, Resp>: Clone {
    /// The error that can happen during send
    type Error: Error + MaybeWrongResponse + From<WrongResponseType> + 'static;
    /// Send a request and receive a response
    ///
    /// # Errors
    /// Returns an error for any of the following cases:
    /// * Failed at the transport layer
    /// * Failed to serialise/deserialise
    /// * Received the wrong type of response
    fn send(&self, request: Req) -> Result<Resp, Self::Error>;
}

/// A simple client which has a transport and format specified
#[derive(Debug, Copy, Clone)]
pub struct SimpleClient<F, T> {
    format: F,
    transport: T,
}

impl<F, T, Req, Resp> AsyncClient<Req, Resp> for SimpleClient<F, T>
where
    F: Format<Resp, Req>,
    T: AsyncTransport,
    Self: Clone
{
    type Error = RpcError<T::Error>;
    /// Send a request and receive a response
    ///
    /// # Errors
    /// Returns an error for any of the following cases:
    /// * Failed at the transport layer
    /// * Failed to serialise/deserialise
    /// * Received the wrong type of response
    async fn send(&self, request: Req) -> Result<Resp, Self::Error> {
        let request = self.format.write(request).map_err(RpcError::Serialize)?;
        let response = self.transport.send(request, self.format.content_type()).await.map_err(RpcError::Transport)??;
        let response = self.format.read(response.as_slice()).map_err(RpcError::Deserialize)?;
        Ok(response)
    }
}

impl<F, T, Req, Resp> BlockingClient<Req, Resp> for SimpleClient<F, T>
where
    F: Format<Resp, Req>,
    T: BlockingTransport,
    Self: Clone
{
    type Error = RpcError<T::Error>;
    fn send(&self, request: Req) -> Result<Resp, Self::Error> {
        let request = self.format.write(request).map_err(RpcError::Serialize)?;
        let response = self.transport.send(request, self.format.content_type()).map_err(RpcError::Transport)??;
        let response = self.format.read(response.as_slice()).map_err(RpcError::Deserialize)?;
        Ok(response)
    }
}

/// This trait describes the transport layer of a client,
///
/// It is responsible for sending the request and waiting for a response
///
/// This is not tied to any particular form of serialisation or communication, nor is it tied to
/// any crate like serde, an individual transport layer may choose what limitations to place on the
/// request/response types
///
/// Naturally a format and protocol the is supported by the server should be chosen
pub trait AsyncTransport: Clone {
    /// This is the error type which is returned in the case that some part of the transport failed
    type Error: Error + 'static;
    /// Sends the request and returns the response
    fn send(&self, request: Vec<u8>, content_type: &str) -> impl Future<Output=Result<Result<Vec<u8>, ResponseError>, Self::Error>>;
}

/// This trait describes the transport layer of a client,
///
/// It is responsible for:
///   * serialising the request
///   * sending the request
///   * waiting for a response
///   * deserialising the response
///
/// This is not tied to any particular form of serialisation or communication, nor is it tied to
/// any crate like serde, an individual transport layer may choose what limitations to place on the
/// request/response types
///
/// Naturally a format and protocol the is supported by the server should be chosen
pub trait BlockingTransport: Clone {
    /// This is the error type which is returned in the case that some part of the transport failed
    type Error: Error + 'static;
    /// Sends the request and returns the response
    ///
    /// # Errors
    /// Returns an error in the case that the communication failed for any reason
    fn send(&self, request: Vec<u8>, content_type: &str) -> Result<Result<Vec<u8>, ResponseError>, Self::Error>;
}

/// This is a transport layer used for nesting services
#[derive(Debug)]
pub struct MappedClient<T, InnerReq, OuterReq, InnerResp, OuterResp, Args> {
    outer: T,
    args: Args,
    to_inner: fn(Result<OuterResp, WrongResponseType>) -> Result<InnerResp, WrongResponseType>,
    to_outer: fn(Args, InnerReq) -> OuterReq,
}

impl<T: Copy, InnerReq, OuterReq, InnerResp, OuterResp, Args: Copy> Copy
for MappedClient<T, InnerReq, OuterReq, InnerResp, OuterResp, Args>
{}

impl<T: Clone, InnerReq, OuterReq, InnerResp, OuterResp, Args: Clone> Clone
for MappedClient<T, InnerReq, OuterReq, InnerResp, OuterResp, Args>
{
    fn clone(&self) -> Self {
        Self {
            outer: self.outer.clone(),
            args: self.args.clone(),
            to_inner: self.to_inner,
            to_outer: self.to_outer,
        }
    }
}

impl<T, InnerReq, OuterReq, InnerResp, OuterResp, Args>
MappedClient<T, InnerReq, OuterReq, InnerResp, OuterResp, Args>
{
    #[doc(hidden)]
    #[must_use]
    pub fn new(
        inner: T,
        args: Args,
        to_inner: fn(Result<OuterResp, WrongResponseType>) -> Result<InnerResp, WrongResponseType>,
        to_outer: fn(Args, InnerReq) -> OuterReq,
    ) -> Self {
        Self {
            outer: inner,
            args,
            to_inner,
            to_outer,
        }
    }
}
impl<T, InnerReq, OuterReq, InnerResp, OuterResp, Args> AsyncClient<InnerReq, InnerResp>
for MappedClient<T, InnerReq, OuterReq, InnerResp, OuterResp, Args>
where
    Args: Clone,
    T: AsyncClient<OuterReq, OuterResp>,
{
    type Error = T::Error;
    async fn send(&self, request: InnerReq) -> Result<InnerResp, Self::Error> {
        let request = (self.to_outer)(self.args.clone(), request);
        let response = match self.outer.send(request).await {
            Ok(response) => Ok(response),
            Err(err) => Err(err.into_wrong_response()?),
        };
        let response = (self.to_inner)(response)?;
        Ok(response)
    }
}

impl<T, InnerReq, OuterReq, InnerResp, OuterResp, Args> BlockingClient<InnerReq, InnerResp>
for MappedClient<T, InnerReq, OuterReq, InnerResp, OuterResp, Args>
where
    Args: Clone,
    T: BlockingClient<OuterReq, OuterResp>,
{
    type Error = T::Error;
    fn send(&self, request: InnerReq) -> Result<InnerResp, Self::Error> {
        let request = (self.to_outer)(self.args.clone(), request);
        let response = match self.outer.send(request) {
            Ok(response) => Ok(response),
            Err(err) => Err(err.into_wrong_response()?),
        };
        let response = (self.to_inner)(response)?;
        Ok(response)
    }
}

/// This is a error that the client may return after a request
#[derive(Debug, Error)]
pub enum RpcError<T> {
    /// The transport layer returned an error
    #[error("Failed to send request: {0}")]
    Transport(#[source] T),
    /// The transport layer returned an error
    #[error("Unexpected response: {0}")]
    Response(#[from] ResponseError),
    /// Failed to serialize the request
    #[error("Failed to serialize the request: {0}")]
    Serialize(Box<dyn Error>),
    /// Failed to deserialize the response
    #[error("Failed to deserialize the response: {0}")]
    Deserialize(Box<dyn Error>),
    /// Response was the wrong type, sent a request for one function, but received the response of a different one
    ///
    /// This is not an expected case and is simply included as an alternative to panicking in this case
    /// This error either means the server side is misbehaving quite badly, or the transport is not configured to the correct endpoint
    #[error(transparent)]
    WrongResponseType(#[from] WrongResponseType),
}

/// Indicates that the transport was successful, but the response indicated some problem
#[derive(Debug, Error, Clone)]
pub enum ResponseError {
    /// Request was improperly formatted
    #[error("Request was rejected: {0}")]
    BadRequest(String),
    /// Internal Server Error
    #[error("Internal Server Error: {0}")]
    InternalServerError(String),
    /// Unexpected response
    #[error("Unexpected response")]
    Unexpected,
}

/// Response was the wrong type: sent a request for one function, but received the response of a different one
///
/// This is not an expected case and is simply included to avoid panicking in this case
/// This error either means the server side is misbehaving quite badly, or the transport is not configured to the correct endpoint
#[derive(Debug, Error, Clone)]
#[error("Response was for the wrong method, sent a request for {expected}, but received the response for {actual}")]
pub struct WrongResponseType {
    /// The expected method
    pub expected: String,
    /// The actual method
    pub actual: String,
}

impl WrongResponseType {
    #[doc(hidden)]
    #[must_use]
    pub fn new(expected: &str, actual: &str) -> Self {
        Self {
            expected: format!("{expected}()"),
            actual: format!("{actual}()"),
        }
    }
    #[doc(hidden)]
    #[must_use]
    pub fn in_subservice(self, name: &str) -> Self {
        Self {
            expected: format!("{name}().{}", self.expected),
            actual: format!("{name}().{}", self.actual),
        }
    }
}

/// An error which might be a [`WrongResponseType`] error
pub trait MaybeWrongResponse: Sized {
    /// Try to cast to a [`WrongResponseType`]
    ///
    /// # Errors
    ///
    /// Returns an Err variant with self if this value is not a [`WrongResponseType`]
    fn into_wrong_response(self) -> Result<WrongResponseType, Self>;
}

impl<T: Error> MaybeWrongResponse for RpcError<T> {
    fn into_wrong_response(self) -> Result<WrongResponseType, Self> {
        if let Self::WrongResponseType(err) = self {
            Ok(err)
        } else {
            Err(self)
        }
    }
}
