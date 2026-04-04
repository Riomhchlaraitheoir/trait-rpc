#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

use std::borrow::Cow;
use std::fmt::Debug;
pub use serde;
pub use futures;

pub mod server;
pub mod client;
pub mod format;
#[cfg(any(feature = "websocket-server", feature = "websocket-client"))]
pub mod stream;

pub use macros::rpc;
pub use crate::client::{AsyncTransport, BlockingTransport, MappedClient, RpcError};
pub use server::Handler;
use crate::client::{AsyncClient, BlockingClient};

/// This is a trait for the main entry point of the RPC, it describes the types for client,
/// request and response
pub trait Rpc: Sized {
    /// This is the async client type used for accessing the RPC service
    type AsyncClient<T: AsyncClient<Self::Request, Self::Response>>;
    /// This is the blocking client type used for accessing the RPC service
    type BlockingClient<T: BlockingClient<Self::Request, Self::Response>>;
    /// This is the request type accepted by the service
    type Request: Request + Debug + 'static;
    /// This is the response type returned by the service
    type Response: Debug + 'static;

    /// Create a new asynchronous client, using the given underlying transport, if you wish to re-use the
    /// client for multiple calls, ensure you pass a copyable transport (eg: a reference)
    fn async_client<C>(transport: C) -> Self::AsyncClient<C>
    where
        C: AsyncClient<Self::Request, Self::Response>;
    /// Create a new blocking client, using the given underlying transport, if you wish to re-use the
    /// client for multiple calls, ensure you pass a copyable transport (eg: a reference)
    fn blocking_client<C>(transport: C) -> Self::BlockingClient<C>
    where
        C: BlockingClient<Self::Request, Self::Response>;

    /// Returns the name of this service
    fn service_name() -> &'static str;
}

/// Represents a [Rpc] which can be served by `Server`
pub trait RpcWithServer<Server>: Rpc {
    /// The handler type for this server
    type Handler: Handler<Rpc = Self>;
    /// Create a new handler from the given server
    fn handler(server: Server) -> Self::Handler;
}

/// Defines a RPC request
pub trait Request {
    /// Returns true if this request has a streaming response
    fn is_streaming_response(&self) -> bool;
    /// Returns the name of this method (for logging purposes)
    fn name(&self) -> Cow<'static, str>;
}
