//! Defines a websocket client

use crate::client::{ResponseError, StreamTransport};
use crate::{get_request_id, prepend_id, AsyncTransport};
use futures::channel::{mpsc, oneshot};
use futures::lock::Mutex;
use futures::{select, SinkExt, Stream, StreamExt};
use std::collections::HashMap;
use std::mem;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::{ClientRequestBuilder, Error as WsError, Message};
use tracing::{error, warn};
use crate::format::IsFormat;

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

/// A client which communicates using a websocket connection
pub struct Websocket {
    sender: RequestSender,
    senders: SenderMap,
    stream_senders: StreamSenderMap,
    content_type: &'static str,
}

type RequestSender = Arc<Mutex<mpsc::Sender<(u32, Vec<u8>)>>>;
type SenderMap = Arc<Mutex<HashMap<u32, oneshot::Sender<Result<Vec<u8>, WebsocketError>>>>>;
type StreamSenderMap = Arc<Mutex<HashMap<u32, mpsc::UnboundedSender<Result<Vec<u8>, WebsocketError>>>>>;

impl Clone for Websocket {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            senders: self.senders.clone(),
            stream_senders: self.stream_senders.clone(),
            content_type: self.content_type,
        }
    }
}

impl Websocket {
    /// Create a new websocket client
    ///
    /// # Errors
    /// Returns an error if the websocket connection could not be opened
    ///
    /// # Panics
    /// Certain unexpected edge cases that cannot be proven safe with the type system may cause a panic
    pub async fn new(url: impl AsRef<str>, format: impl IsFormat) -> Result<Self, WsError> {
        let (mut stream, _) =
            connect_async(ClientRequestBuilder::new(url.as_ref().parse().expect("failed to parse url")).with_sub_protocol(format.content_type()))
                .await?;
        let (sender, mut request_receiver) = mpsc::channel(100);
        let sender = Arc::new(Mutex::new(sender));
        let senders: SenderMap = Arc::default();
        let stream_senders: StreamSenderMap = Arc::default();
        tokio::spawn({
            let response_senders = senders.clone();
            let stream_senders = stream_senders.clone();
            async move {
                let closed: bool = 'worker: loop {
                    select! {
                    req = request_receiver.next() => {
                        let Some((request_id, request)) = req else {
                            continue 'worker;
                        };
                        let request = prepend_id(request_id, request);
                        if let Err(error) = stream.send(Message::Binary(request.into())).await {
                            warn!("Error sending message: {}", error);
                            break 'worker false;
                        }
                    },
                    response = stream.next() => {
                            let response = match response {
                                Some(Ok(message)) => {
                                    match message {
                                        Message::Text(error) => {
                                            warn!("Error from server: {}", error);
                                            continue 'worker;
                                        }
                                        Message::Binary(response) => response,
                                        Message::Ping(bytes) => {
                                            if let Err(error) = stream.send(Message::Pong(bytes)).await {
                                                warn!("Error sending pong message: {}", error);
                                            }
                                            continue 'worker;
                                        }
                                        Message::Pong(_) => {
                                            continue 'worker;
                                        }
                                        Message::Close(_) => {
                                            let _: Result<(), _> = stream.send(Message::Close(None)).await;
                                            break 'worker true;
                                        }
                                        Message::Frame(_) => unreachable!("Cannot receive raw data frame"),
                                    }
                                }
                                Some(Err(error)) => {
                                    error!("Error from websocket connection: {}", error);
                                    continue 'worker;
                                }
                                None => {
                                    warn!("websocket closed");
                                    break 'worker false;
                                }
                            };
                            let (request_id, response) = get_request_id(&response);
                            if let Some(sender) = response_senders.lock().await.remove(&request_id) {
                                let _: Result<(), _> = sender.send(Ok(response.to_vec()));
                            } else if let Some(sender) = stream_senders.lock().await.get_mut(&request_id) {
                                let _: Result<(), _> = sender.send(Ok(response.to_vec())).await;
                            } else {
                                panic!("no sender found for request: {request_id}");
                            }
                    }
                    }
                };
                if !closed {
                    let _: Result<(), _> = stream.send(Message::Close(None)).await;
                }
                let senders = mem::take(&mut *response_senders.lock().await);
                for (_, sender) in senders {
                    let _ = sender.send(Err(WebsocketError::ConnectionClosed));
                }
            }
        });
        Ok(Self {
            sender,
            senders,
            stream_senders,
            content_type: format.content_type(),
        })
    }
}

impl AsyncTransport for Websocket {
    type Error = WebsocketError;

    async fn send(&self, request: Vec<u8>, content_type: &str) -> Result<Result<Vec<u8>, ResponseError>, Self::Error> {
        if self.content_type != content_type {
            return Err(WebsocketError::IncorrectContentType {
                expected: self.content_type,
                received: content_type.to_string(),
            })
        }
        let (sender, receiver) = oneshot::channel();
        let request_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        self.senders
            .lock()
            .await
            .insert(request_id, sender);
        self.sender
            .lock()
            .await
            .send((request_id, request))
            .await
            .map_err(|_| WebsocketError::RequestChannelClosed)?;
        Ok(Ok(receiver
            .await
            .map_err(|_| WebsocketError::ResponseChannelClosed)??))
    }
}

impl StreamTransport for Websocket {
    async fn stream_resp(&self, request: Vec<u8>, content_type: &str) -> Result<impl Stream<Item=Result<Vec<u8>, Self::Error>>, WebsocketError> {
        if self.content_type != content_type {
            return Err(WebsocketError::IncorrectContentType {
                expected: self.content_type,
                received: content_type.to_string(),
            })
        }
        let (sender, receiver) = mpsc::unbounded();
        let request_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        self.stream_senders
            .lock()
            .await
            .insert(request_id, sender);
        self.sender
            .lock()
            .await
            .send((request_id, request))
            .await
            .map_err(|_| WebsocketError::RequestChannelClosed)?;
        Ok(receiver)
    }
}

/// An error from the websocket client
#[derive(Debug, Error)]
pub enum WebsocketError {
    /// The websocket worker has closed the request channel, this is not expected
    #[error("Failed to send request to worker: channel closed")]
    RequestChannelClosed,
    /// The websocket worker has closed the response channel, this is not expected
    #[error("Failed to read response from worker: channel closed")]
    ResponseChannelClosed,
    /// The client is not using the same content type as the websocket transport
    #[error("The client is not using the same content type as the websocket transport, expected: {expected}, received: {received}")]
    IncorrectContentType {
        /// The content type defined in the websocket transport
        expected: &'static str,
        /// The content type defined in the client
        received: String,
    },
    /// The websocket connection has closed
    #[error("Websocket connection closed")]
    ConnectionClosed,
}
