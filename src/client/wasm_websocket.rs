//! Defines a websocket client

use crate::format::IsFormat;
use crate::stream::client::StreamClient;
use futures::{SinkExt, StreamExt};
use gloo_net::websocket::futures::WebSocket;
use gloo_net::websocket::{Message, WebSocketError};
use std::future::ready;
use thiserror::Error;
use tracing::{debug, error};
use wasm_bindgen_futures::spawn_local;

/// Error from websocket
#[derive(Debug, Error)]
pub enum WebsocketError {
    /// Failed to connect to websocket
    #[error("Failed to connect: {0}")]
    ConnectionError(String),
}

/// Create a new websocket transport layer to the given URL
///
/// # Errors
/// Returns an error if an error occurs while establishing a connection
#[expect(clippy::unused_async, reason = "might be needed in the future")]
pub async fn new_websocket_transport(
    url: impl AsRef<str>,
    format: impl IsFormat + 'static,
) -> Result<StreamClient, WebsocketError> {
    debug!("Connecting websocket to {}", url.as_ref());
    let socket = WebSocket::open_with_protocol(url.as_ref(), format.subprotocol())
        .map_err(|e| WebsocketError::ConnectionError(e.message))?;

    let (request_sink, response_stream) = socket.split();
    let request_sink =
        request_sink.with(|data| {
            ready(Result::<_, WebSocketError>::Ok(Message::Bytes(data)))
        });
    let response_stream = response_stream.filter_map(|message| {
        ready(match message {
            Ok(Message::Text(text)) => {
                error!("Text frames not supported, text: {text}");
                None
            }
            Ok(Message::Bytes(data)) => Some(data),
            Err(error) => {
                error!("received websocket error: {error}");
                None
            }
        })
    });

    let (client, job) = StreamClient::new(request_sink, response_stream, format);
    spawn_local(async {
        if let Err(error) = job.await {
            error!("Error on websocket connection: {error}");
        }
    });
    Ok(client)
}
