//! Defines a websocket client

use crate::client::ResponseError;
use crate::format::IsFormat;
use crate::{get_request_id, prepend_id, AsyncTransport};
use futures::channel::{mpsc, oneshot};
use futures::lock::Mutex;
use futures::{select, FutureExt, SinkExt, StreamExt};
use std::collections::HashMap;
use std::mem;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tracing::warn;
use wasm_bindgen_futures::spawn_local;
use ws_stream_wasm::{CloseEvent, WsErr, WsMessage, WsMeta};

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

/// A client which communicates using a websocket connection
pub struct Websocket {
    sender: RequestSender,
    senders: SenderMap,
    content_type: &'static str
}

type RequestSender = Arc<Mutex<mpsc::Sender<(u32, Vec<u8>)>>>;
type SenderMap = Arc<Mutex<HashMap<u32, oneshot::Sender<Result<Vec<u8>, WebsocketError>>>>>;

impl Clone for Websocket {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            senders: self.senders.clone(),
            content_type: self.content_type
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
    pub async fn new(
        url: impl AsRef<str>,
        format: impl IsFormat,
    ) -> Result<Self, WsErr> {
        let (meta, mut stream) = WsMeta::connect(url, Some(vec![format.content_type()])).await?;
        let (sender, mut request_receiver) = mpsc::channel(100);
        let sender = Arc::new(Mutex::new(sender));
        let senders: SenderMap = Arc::default();
        spawn_local({
            let response_senders = senders.clone();
            async move {
                let closed: bool = 'worker: loop {
                    select! {
                                        req = request_receiver.next() => {
                                            let Some((request_id, request)) = req else {
                                                continue 'worker;
                                            };
                                            let request = prepend_id(request_id, request);
                                            if let Err(error) = stream.send(WsMessage::Binary(request)).await {
                                                warn!("Error sending message: {}", error);
                                                break 'worker false;
                                            }
                                        },
                                        response = stream.next().fuse() => {
                                                let response = if let Some(message) = response {
                                                        match message {
                                                            WsMessage::Text(error) => {
                                                                warn!("Error from server: {}", error);
                                                                continue 'worker;
                                                            }
                                                            WsMessage::Binary(response) => response,
                                                        }
                                                } else {
                                                    warn!("websocket closed");
                                                    break 'worker false;
                                                };
                                                let (request_id, response) = get_request_id(&response);
                                                let sender = response_senders.lock().await.remove(&request_id).expect("sender not found");
                                                let _: Result<(), _> = sender.send(Ok(response.to_vec()));
                                        }
                                        }
                };
                if !closed {
                    let _: Result<CloseEvent, _> = meta.close().await;
                }
                let senders = mem::take(&mut *response_senders.lock().await);
                for (_, sender) in senders {
                    let _ = sender.send(Err(WebsocketError::ConnectionClosed));
                }
            }
        });
        Ok(Self { sender, senders, content_type: format.content_type() })
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
        self.senders.lock().await.insert(request_id, sender);
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

/// An error from the websocket client
#[derive(Debug, Error)]
pub enum WebsocketError {
    /// The websocket worker has closed the request channel, this is not expected
    #[error("Failed to send request to worker: channel closed")]
    RequestChannelClosed,
    /// The websocket worker has closed the response channel, this is not expected
    #[error("Failed to read response from worker: channel closed")]
    ResponseChannelClosed,
    /// The websocket connection has closed
    #[error("Websocket connection closed")]
    ConnectionClosed,
    /// The client is not using the same content type as the websocket transport
    #[error("The client is not using the same content type as the websocket transport, expected: {expected}, received: {received}")]
    IncorrectContentType {
        /// The content type defined in the websocket transport
        expected: &'static str,
        /// The content type defined in the client
        received: String,
    },
}
