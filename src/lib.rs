#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

pub use serde;
pub use futures;

pub mod server;
pub mod client;
pub mod format;

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
    type Request: Request + 'static;
    /// This is the response type returned by the service
    type Response: 'static;

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
}

#[allow(dead_code, reason = "only using in certain features, but better to leave it open")]
/// Build a request/response from a request ID and a payload. Useful for implementing transport
/// protocols that share a single connection for many concurrent requests
fn prepend_id(request_id:u32, payload: Vec<u8>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(payload.len() + 4);
    bytes.extend(request_id.to_le_bytes());
    bytes.extend(payload);
    bytes
}

#[allow(dead_code, reason = "only using in certain features, but better to leave it open")]
/// Get request id and payload from the given request/response. Useful for implementing transport
/// protocols that share a single connection for many concurrent requests
fn get_request_id(request: &[u8]) -> (u32, &[u8]) {
    let request_id = u32::from_le_bytes([
        request[0],
        request[1],
        request[2],
        request[3],
    ]);
    (request_id, &request[4..])
}
