//! Defines a websocket client

use crate::format::IsFormat;
use crate::stream::client::StreamClient;
use futures::{SinkExt, StreamExt};
use futures::channel::mpsc::unbounded;
use futures::future::{select, Either};
use tokio_tungstenite::tungstenite::{ClientRequestBuilder, Error as WsError, Error, Message};
use tokio_tungstenite::{connect_async};
use tracing::{error, info, info_span, Instrument};
use std::pin::pin;

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
) -> Result<StreamClient, WsError> {
    let (mut websocket, _) = connect_async(
        ClientRequestBuilder::new(url.as_ref().parse().expect("failed to parse url"))
            .with_sub_protocol(format.content_type()),
    )
    .await?;
    let (mut request_sender, request_receiver) = unbounded();
    let (response_sender, mut response_receiver) = unbounded();
    let (client, job) = StreamClient::new(response_sender, request_receiver, format);
    tokio::spawn(async move {
        loop {
            let next_request = websocket.next();
            let next_request = pin!(next_request);
            let next_response = response_receiver.next();
            let next_response = pin!(next_response);
            match select(next_response, next_request).await {
                Either::Left((None, _)) => break,
                Either::Left((Some(bytes), _)) => {
                    let result = websocket.send(Message::binary(bytes)).await;
                    if let Err(error) = result {
                        error!("failed to send response, closing websocket: {error}");
                        break
                    }
                }
                Either::Right((None, _)) => return, // ended by client
                Either::Right((Some(request), _)) => {
                    match parse_message(request) {
                        None => {},
                        Some(Either::Left(bytes)) => {
                            if let Err(error) = request_sender.send(bytes).await {
                                error!("failed to send parsed request, closing websocket: {error}");
                                break
                            }
                        },
                        Some(Either::Right(message)) => {
                            if let Err(error) = websocket.send(message).await {
                                error!("failed to send message, closing websocket: {error}");
                                break
                            }
                        }
                    }
                }
            }
        }
    }.instrument(info_span!("websocket message parser")));
    tokio::spawn(job);
    Ok(client)
}

fn parse_message(result: Result<Message, Error>) -> Option<Either<Vec<u8>, Message>> {
    let message = match result {
        Ok(message) => message,
        Err(error) => {
            info!("Websocket disconnected with error: {error}");
            return None;
        }
    };
    match message {
        Message::Text(text) => {
            error!("Received unexpected text message: {text}");
            None
        },
        Message::Binary(bytes) => {
            Some(Either::Left(bytes.into()))
        },
        Message::Ping(bytes) => Some(Either::Right(Message::Pong(bytes))),
        Message::Pong(_) => None,
        Message::Close(frame) => {
            if let Some(frame) = frame {
                info!("Websocket connection closed, code: {}, reason: {}", frame.code, frame.reason);
            } else {
                info!("Websocket connection closed without frame");
            }
            Some(Either::Right(Message::Close(None)))
        }
        Message::Frame(_) => unreachable!("Should never receive a raw message")
    }
}
