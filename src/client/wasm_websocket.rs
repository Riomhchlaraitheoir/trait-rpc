//! Defines a websocket client

use crate::client::StreamTransport;
use crate::format::IsFormat;
use crate::stream::client::StreamClient;
use futures::{Sink, SinkExt, StreamExt};
use tracing::error;
use wasm_bindgen_futures::spawn_local;
use ws_stream_wasm::{WsErr, WsMessage, WsMeta, WsStream};
pub use ws_stream_wasm::WsErr as WebsocketError;

/// Create a new websocket transport layer to the given URL
///
/// # Errors
/// Returns an error if an error occurs while establishing a connection
#[allow(
    clippy::missing_panics_doc,
    reason = "panic in theoretically unreachable"
)]
pub async fn new_websocket_transport(
    url: impl AsRef<str>,
    format: impl IsFormat + 'static,
) -> Result<impl StreamTransport, WsErr> {
    let (_meta, stream) = WsMeta::connect(url, Some(vec![format.content_type()])).await?;
    let (sink, stream) = stream.split();
    let stream = stream.filter_map(|message| async {
        match message {
            WsMessage::Text(text) => {
                error!("Received unexpected text frame from server: {text}");
                None
            },
            WsMessage::Binary(response) => Some(response)
        }
    });
    let sink = sink.with(async |request| {
        Result::<WsMessage, <WsStream as Sink<WsMessage>>::Error>::Ok(WsMessage::Binary(request))
    });
    let (client, job) = StreamClient::new(sink, stream, format);
    spawn_local(async {
        if let Err(error) = job.await {
            error!("Error on websocket connection: {error}");
        }
    });
    Ok(client)
}
