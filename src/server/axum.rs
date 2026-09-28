#[allow(unused_imports, reason = "only used if certain features are enabled")]
use crate::format;
use crate::format::Format;
use crate::{Handler, Rpc, RpcWithServer};
use axum::body::Bytes;
use axum::http::header::{ToStrError, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use std::fmt::Debug;
use thiserror::Error;

#[allow(
    unused_imports,
    reason = "may be unused depending on features, not worth splitting behind toggles"
)]
use tracing::{Instrument, debug, error, info, info_span, warn};
#[cfg(feature = "websocket-server")]
use {
    crate::{format::IsFormat, stream::server::{serve_request_stream, ConnectionHooks}},
    axum::extract::{
        WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    futures::{
        Sink, Stream, StreamExt,
        future::{Either, ready},
        lock::BiLock,
        stream::once,
        FutureExt
    },
    std::{pin::pin, task::Poll},
};

/// Handle an incoming HTTP service request:
/// - Parses the request
/// - Passes request to appropriate server function
/// - Serialises the response
///
/// # Errors
/// May fail to handle the request
pub async fn handle_request<R, Server>(
    server: Server,
    headers: HeaderMap,
    bytes: Bytes,
    formats: &'static [&'static dyn Format<<R as Rpc>::Request, <R as Rpc>::Response>],
) -> Result<Response, HandleError>
where
    R: RpcWithServer<Server>,
{
    let handler = R::handler(server);
    let content_type = headers
        .get("Content-Type")
        .ok_or(HandleError::NoContentType)?
        .to_str()?
        .split(';')
        .next()
        .ok_or(HandleError::NoContentType)?;

    let format = formats
        .iter()
        .find(|format| format.content_type() == content_type)
        .ok_or(HandleError::UnsupportedContentType(content_type.to_string()))?;

    let request = format
        .read(&bytes)
        .map_err(HandleError::Deserialise)?;
    let response = handler.handle(request).await;
    let response = format
        .write(response)
        .map_err(HandleError::Serialise)?;
    Ok((
        StatusCode::OK,
        [(CONTENT_TYPE, format.content_type())],
        response,
    )
        .into_response())
}

/// Error while handling HTTP request
#[derive(Debug, Error)]
pub enum HandleError {
    /// Content-Type header was absent
    #[error("No 'Content-Type' header found")]
    NoContentType,
    /// Content-Type header contained invalid bytes. See [`ToStrError`]
    #[error("Failed to decode Content-Type value: {0}")]
    InvalidContentType(#[from] ToStrError),
    /// Content-Type did not match any provided format
    #[error("Unsupported Content-Type: {0}")]
    UnsupportedContentType(String),
    #[error("Failed to deserialise request: {0}")]
    /// An Error occurred while deserialising the request
    Deserialise(Box<dyn std::error::Error + Send>),
    #[error("Failed to serialise response: {0}")]
    /// An Error occurred while serialising the response
    Serialise(Box<dyn std::error::Error + Send>),
}

/// Handle an incoming websocket connection
///
/// Handle this connection, waiting for requests and sending responses
/// The server must implement [`ServiceBookends`](super::ServiceBookends), this trait controls
/// if and when idle connections are killed
///
/// # Errors
/// May fail to handle the connection
#[cfg(feature = "websocket-server")]
pub fn handle_websocket<R, Server>(
    ws: WebSocketUpgrade,
    server: Server,
    formats: &'static [&'static dyn Format<<R as Rpc>::Request, <R as Rpc>::Response>],
) -> Result<Response, WebsocketError>
where
    R: RpcWithServer<Server>,
    <R as RpcWithServer<Server>>::Handler: Sync + 'static,
    <R as Rpc>::Request: Send,
    <R as Rpc>::Response: Send + Sync,
    Server: ConnectionHooks<R>
{
    let handler = R::handler(server);
    let protocols: Vec<_> = formats.iter().copied().map(IsFormat::subprotocol).collect();
    let ws = ws.protocols(protocols);
    let protocol = ws
        .selected_protocol()
        .ok_or(WebsocketError::UnsupportedFormat)?;
    let format = formats
        .iter()
        .find(|format| format.subprotocol() == protocol)
        .ok_or(WebsocketError::UnsupportedFormat)?;
    let format = *format;
    Ok(ws.on_upgrade(move |websocket| {
        let (sink_lock, stream_lock) = BiLock::new(websocket);
        let stream = websocket_stream(stream_lock);
        let sink = websocket_sink(sink_lock);
        serve_request_stream(stream, sink, handler, format).map(|result| {
            if let Err(error) = result {
                error!("Error occurred on websocket connection: {error}");
            }
        })
    }))
}

#[derive(Debug, Error)]
/// An Error which may occur when accepting websocket connections
pub enum WebsocketError {
    #[error("Unsupported format")]
    /// The websocket protocol specifies an unsupported format
    UnsupportedFormat,
}

#[cfg(feature = "websocket-server")]
fn websocket_stream(websocket: BiLock<WebSocket>) -> impl Stream<Item = Vec<u8>> {
    futures::stream::unfold(websocket, |websocket| async {
        // in order to prevent lcok contention, the future should grab the lock opn each poll,
        // but not hold it between polls, giving ample opportunity for the sender to grab the lock
        let next = futures::future::poll_fn({
            |cx| {
                let Poll::Ready(mut lock) = pin!(websocket.lock()).as_mut().poll(cx) else {
                    return Poll::Pending;
                };
                pin!(lock.next()).poll(cx)
            }
        });
        let next = next.await?;
        let message = match next {
            Ok(message) => message,
            Err(error) => {
                info!("Websocket disconnected with error: {error}");
                return None;
            }
        };
        let mut should_close = false;
        let result = match message {
            Message::Text(_) => {
                websocket
                    .lock()
                    .await
                    .send(Message::Text("text frames not supported".into()))
                    .await
            }
            Message::Binary(bytes) => {
                return Some((Some(bytes.to_vec()), websocket));
            }
            Message::Ping(bytes) => websocket.lock().await.send(Message::Pong(bytes)).await,
            Message::Pong(_) => Ok(()),
            Message::Close(frame) => {
                if let Some(frame) = frame {
                    info!(
                        "Websocket connection closed, code: {}, reason: {}",
                        frame.code, frame.reason
                    );
                } else {
                    info!("Websocket connection closed without frame");
                }
                should_close = true;
                websocket.lock().await.send(Message::Close(None)).await
            }
        };
        if let Err(error) = result {
            error!("Websocket connection error: {error}");
            return None;
        }
        if should_close {
            None
        } else {
            Some((None, websocket))
        }
    })
    .flat_map(|option| {
        option.map_or_else(
            || Either::Right(futures::stream::empty()),
            |value| Either::Left(once(ready(value))),
        )
    })
}

#[cfg(feature = "websocket-server")]
fn websocket_sink(
    websocket: BiLock<WebSocket>,
) -> impl Sink<Vec<u8>, Error = <WebSocket as Sink<Message>>::Error> {
    futures::sink::unfold(websocket, |websocket, bytes| async {
        let result = {
            debug!("Acquiring lock for websocket sink");
            let mut sink = websocket.lock().await; // FIXME: lock contention
            debug!("Sending response: {bytes:?}");
            sink.send(Message::binary(bytes)).await
        };
        if let Err(error) = result {
            warn!("Failed to send response: {error}");
            Err(error)
        } else {
            debug!("Websocket sent successfully");
            Ok(websocket)
        }
    })
}
