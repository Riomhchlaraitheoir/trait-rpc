//! Helpers for dealing with stream connections on the client side

use crate::AsyncTransport;
use crate::client::{HandleError, StreamTransport};
use crate::format::IsFormat;
use crate::stream::{ConnectionError, ConnectionMessage, message_sink, message_stream};
use futures::channel::{mpsc, oneshot};
use futures::lock::Mutex;
use futures::{Sink, SinkExt, Stream, StreamExt, FutureExt};
use std::collections::HashMap;
use std::mem;
use std::pin::{pin, Pin};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use futures::future::join_all;
use thiserror::Error;
use tracing::{Instrument, debug, info_span, warn};

/// A client which communicates using a websocket connection
#[derive(Debug, Clone)]
pub struct StreamClient {
    sender: RequestSender,
    senders: SenderMap,
    stream_senders: StreamSenderMap,
    content_type: &'static str,
    next_id: Arc<AtomicU32>,
}

type RequestSender = Arc<Mutex<mpsc::Sender<ConnectionMessage<Vec<u8>>>>>;
type SenderMap = Arc<Mutex<HashMap<u32, oneshot::Sender<Result<Vec<u8>, StreamError>>>>>;
type StreamSenderMap =
    Arc<Mutex<HashMap<u32, mpsc::UnboundedSender<Result<Vec<u8>, StreamError>>>>>;

/// An error that may occur while communicating with a server over a stream
#[derive(Debug, Error)]
pub enum ClientError<E> {
    /// The sink failed to send
    #[error(transparent)]
    Sink(#[from] E),
    /// An error occurred on the connection
    #[error(transparent)]
    ConnectionError(ConnectionError),
}

impl StreamClient {
    /// Create a new streaming client
    ///
    /// Returns the client `Self` and a future which should be run concurrently to manage the connection
    ///
    /// # Errors
    /// Returns an error if the websocket connection could not be opened
    ///
    /// # Panics
    /// Certain unexpected edge cases that cannot be proven safe with the type system may cause a panic
    #[allow(clippy::needless_pass_by_value)]
    pub fn new<Out, In>(
        request_sink: Out,
        response_stream: In,
        format: impl IsFormat,
    ) -> (
        Self,
        impl Future<Output = Result<(), ClientError<Out::Error>>> + Send + 'static,
    )
    where
        Out: Sink<Vec<u8>> + Send + 'static,
        In: Stream<Item = Vec<u8>> + Send + 'static,
        Out::Error: Send,
    {
        let (sender, request_receiver) = mpsc::channel(100);
        let sender: RequestSender = Arc::new(Mutex::new(sender));
        let senders: SenderMap = Arc::default();
        let stream_senders: StreamSenderMap = Arc::default();
        let request_sender = Self::request_sender(request_receiver, request_sink).instrument(info_span!("client request handler"));
        let response_handler = Self::response_handler::<Out::Error>(response_stream, stream_senders.clone(), senders.clone()).instrument(info_span!("client response handler"));
        let job = join_all([
            Box::pin(request_sender) as Pin<Box<dyn Future<Output = Result<(), ClientError<Out::Error>>> + Send>>,
            Box::pin(response_handler),
        ]).map(|results| results.into_iter().collect());
        let client = Self {
            sender,
            senders,
            stream_senders,
            content_type: format.content_type(),
            next_id: Arc::new(AtomicU32::new(0)),
        };
        (client, job)
    }

    async fn request_sender<Out>(
        mut request_receiver: mpsc::Receiver<ConnectionMessage<Vec<u8>>>,
        request_sink: Out,
    ) -> Result<(), ClientError<Out::Error>> where
        Out: Sink<Vec<u8>>,
    {
        let mut request_sink = pin!(message_sink(request_sink, "client"));
        while let Some(message) = request_receiver.next().await {
            request_sink.send(message).await?;
        }
        Ok(())
    }

    async fn response_handler<E>(response_stream: impl Stream<Item = Vec<u8>>, stream_senders: StreamSenderMap, response_senders: SenderMap) -> Result<(), ClientError<E>> {
        let mut response_stream = pin!(message_stream(response_stream, "client").fuse());
        loop {
            let response = response_stream.next().await;
            let Some(message) = response else {
                warn!("websocket closed");
                break;
            };
            debug!("Received message: {:?}", message);
            let (request_id, response) = match message {
                ConnectionMessage::Payload {
                    request_id,
                    payload,
                } => (request_id, Ok(payload)),
                ConnectionMessage::ConnectionError(error) => {
                    return Err(ClientError::ConnectionError(error));
                }
                ConnectionMessage::HandleError { request_id, error } => {
                    (request_id, Err(error))
                }
                ConnectionMessage::StreamEnd { request_id } => {
                    if let Some(mut sender) =
                        stream_senders.lock().await.remove(&request_id)
                    {
                        sender.disconnect();
                    } else {
                        warn!("sender not found for request: {request_id}");
                    }
                    continue;
                }
            };
            let result = response.map_err(StreamError::from);
            if let Some(sender) = response_senders.lock().await.remove(&request_id) {
                let _: Result<(), _> = sender.send(result);
            } else if let Some(sender) = stream_senders.lock().await.get_mut(&request_id) {
                let _: Result<(), _> = sender.send(result).await;
            } else {
                panic!("no sender found for request: {request_id}");
            }
        }
        let senders = mem::take(&mut *response_senders.lock().await);
        for (_, sender) in senders {
            let _ = sender.send(Err(StreamError::ConnectionClosed));
        }
        Ok(())
    }
}

impl AsyncTransport for StreamClient {
    type Error = StreamError;

    async fn send(
        &self,
        request: Vec<u8>,
        content_type: &str,
    ) -> Result<Result<Vec<u8>, HandleError>, <Self as AsyncTransport>::Error> {
        if self.content_type != content_type {
            return Err(StreamError::IncorrectContentType {
                expected: self.content_type,
                received: content_type.to_string(),
            });
        }
        let (sender, receiver) = oneshot::channel();
        let request_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.senders.lock().await.insert(request_id, sender);
        self.sender
            .lock()
            .await
            .send(ConnectionMessage::Payload {
                request_id,
                payload: request,
            })
            .await
            .map_err(|_| StreamError::RequestChannelClosed)?;
        Ok(Ok(receiver
            .await
            .map_err(|_| StreamError::ResponseChannelClosed)??))
    }
}

impl StreamTransport for StreamClient {
    async fn stream_resp(
        &self,
        request: Vec<u8>,
        content_type: &str,
    ) -> Result<impl Stream<Item = Result<Vec<u8>, Self::Error>>, StreamError> {
        if self.content_type != content_type {
            return Err(StreamError::IncorrectContentType {
                expected: self.content_type,
                received: content_type.to_string(),
            });
        }
        let (sender, receiver) = mpsc::unbounded();
        let request_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.stream_senders.lock().await.insert(request_id, sender);
        self.sender
            .lock()
            .await
            .send(ConnectionMessage::Payload {
                request_id,
                payload: request,
            })
            .await
            .map_err(|_| StreamError::RequestChannelClosed)?;
        Ok(receiver)
    }
}

/// An error from the websocket client
#[derive(Debug, Error)]
pub enum StreamError {
    /// The websocket worker has closed the request channel, this is not expected
    #[error("Failed to send request to worker: channel closed")]
    RequestChannelClosed,
    /// The websocket worker has closed the response channel, this is not expected
    #[error("Failed to read response from worker: channel closed")]
    ResponseChannelClosed,
    /// The client is not using the same content type as the websocket transport
    #[error(
        "The client is not using the same content type as the websocket transport, expected: {expected}, received: {received}"
    )]
    IncorrectContentType {
        /// The content type defined in the websocket transport
        expected: &'static str,
        /// The content type defined in the client
        received: String,
    },
    /// The websocket connection has closed
    #[error("Websocket connection closed")]
    ConnectionClosed,
    /// An error was received from the server
    #[error("Error from server: {0}")]
    ServerError(#[from] HandleError),
}
