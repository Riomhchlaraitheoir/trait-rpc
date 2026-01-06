#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

pub use serde;

pub mod server;
pub mod client;
pub mod format;

pub use macros::rpc;
pub use crate::client::{AsyncTransport, BlockingTransport, MappedClient, LinkError};
pub use server::Handler;
use crate::client::{AsyncClient, BlockingClient};

/// This is a trait for the main entry point of the RPC, it describes the types for client,
/// request and response
pub trait Rpc: Sync + Sized {
    /// This is the async client type used for accessing the RPC service
    type AsyncClient<T: AsyncClient<Self::Request, Self::Response>>;
    /// This is the blocking client type used for accessing the RPC service
    type BlockingClient<T: BlockingClient<Self::Request, Self::Response>>;
    /// This is the request type accepted by the service
    type Request: Send + 'static;
    /// This is the response type returned by the service
    type Response: Send + 'static;

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
