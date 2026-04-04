//! Module for handling bidirectional connections

use futures::StreamExt;
use futures::{Sink, SinkExt, Stream};
use thiserror::Error;
use tracing::debug;
use crate::client::HandleError;

#[cfg(feature = "websocket-client")]
pub mod client;
#[cfg(feature = "websocket-server")]
pub mod server;

#[derive(Debug)]
enum ConnectionMessage<T> {
    ConnectionError(ConnectionError),
    Payload {
        request_id: u32,
        payload: T,
    },
    HandleError {
        request_id: u32,
        error: HandleError,
    },
    StreamEnd {
        request_id: u32,
    },
}

/// An error on a connection which is not related to any particular request
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ConnectionError {
    /// Internal error: the response channel has closed when it should have remained open
    #[error("Response channel closed unexpectedly")]
    ResponseChannelClosed,
    #[error("Message packet improperly formatted")]
    /// This indicates that a malformed message error was received from the connection.
    /// This should never be sent back into the connection since that could cause an infinite
    /// loop with errors being sent back and forth on the connection.
    /// This will never be read from a connections
    MalformedMessageError,
    /// This indicates that a message was received with invalid formatting
    #[error("Message packet improperly formatted")]
    MessageWasMalformed
}

#[cfg(feature = "websocket-server")]
impl<T> ConnectionMessage<T> {
    fn map<F: Fn(T) -> Result<V, E>, V, E>(self, f: F) -> Result<ConnectionMessage<V>, (u32, E)> {
        Ok(match self {
            Self::ConnectionError(error) => {
                ConnectionMessage::ConnectionError(error)
            }
            Self::Payload { request_id, payload } => {
                match f(payload) {
                    Ok(payload) => ConnectionMessage::Payload { request_id, payload },
                    Err(error) => return Err((request_id, error)),
                }
            }
            Self::HandleError { request_id, error } => {
                ConnectionMessage::HandleError { request_id, error }
            }
            Self::StreamEnd { request_id } => {
                ConnectionMessage::StreamEnd { request_id }
            }
        })
    }

    fn payload<V>(self) -> Result<(u32, T), ConnectionMessage<V>> {
        match self {
            Self::ConnectionError(error) => {
                Err(ConnectionMessage::ConnectionError(error))
            }
            Self::Payload { request_id, payload } => {
                Ok((request_id, payload))
            }
            Self::HandleError { request_id, error } => {
                Err(ConnectionMessage::HandleError { request_id, error })
            }
            Self::StreamEnd { request_id } => {
                Err(ConnectionMessage::StreamEnd { request_id })
            }
        }
    }

    const fn connection_error(&self) -> Option<&ConnectionError> {
        if let Self::ConnectionError(error) = self {
            Some(error)
        } else {
            None
        }
    }
}

#[allow(deprecated)]
impl ConnectionMessage<Vec<u8>> {
    fn format(self) -> Option<Vec<u8>> {
        let mut message = Vec::new();
        match self {
            Self::ConnectionError(error) => {
                message.push(1);
                match error {
                    ConnectionError::ResponseChannelClosed => {
                        message.push(0);
                    }
                    ConnectionError::MessageWasMalformed => {
                        message.push(1);
                    }
                    ConnectionError::MalformedMessageError => {
                        return None;
                    }
                }
            }
            Self::Payload { request_id, payload } => {
                message.push(0);
                message.extend(request_id.to_le_bytes());
                message.extend(&*payload);
            }
            Self::HandleError { request_id, error } => {
                message.push(2);
                message.extend(request_id.to_le_bytes());
                match error {
                    HandleError::BadRequest(error) => {
                        message.push(0);
                        message.extend(error.into_bytes());
                    }
                    HandleError::InternalServerError(error) => {
                        message.push(1);
                        message.extend(error.into_bytes());
                    }
                    HandleError::Unexpected => {
                        message.push(2);
                    }
                }
            }
            Self::StreamEnd { request_id } => {
                message.push(3);
                message.extend(request_id.to_le_bytes());
            }
        }
        Some(message)
    }

    /// Get request id and payload from the given request/response. Useful for implementing transport
    /// protocols that share a single connection for many concurrent requests
    fn parse(message: &[u8]) -> Self {
        let Some(&message_type) = message.first() else {
            return Self::ConnectionError(ConnectionError::MessageWasMalformed);
        };
        let message = &message[1..];
        if message_type == 1 {
            let error_type = message[0];
            return match error_type {
                0 => Self::ConnectionError(ConnectionError::ResponseChannelClosed),
                1 => Self::ConnectionError(ConnectionError::MalformedMessageError),
                _ => Self::ConnectionError(ConnectionError::MessageWasMalformed),
            }
        }
        if message.len() < 4 {
            return Self::ConnectionError(ConnectionError::MessageWasMalformed);
        }
        let request_id = u32::from_le_bytes([message[0], message[1], message[2], message[3]]);
        let message = &message[4..];
        match message_type {
            0 => Self::Payload {
                request_id,
                payload: message.to_vec(),
            },
            2 => {
                let error_type = &message[0];
                let message = &message[1..];
                let error = match *error_type {
                    0 | 1 => {
                        let error = String::from_utf8_lossy(message);
                        match error_type {
                            0 => HandleError::BadRequest(error.into_owned()),
                            1 => HandleError::InternalServerError(error.into_owned()),
                            _ => unreachable!(),
                        }
                    }
                    2 => HandleError::Unexpected,
                    _ => {
                        return Self::ConnectionError(ConnectionError::MessageWasMalformed);
                    }
                };
                Self::HandleError { request_id, error }
            }
            3 => Self::StreamEnd { request_id },
            _ => Self::ConnectionError(ConnectionError::MessageWasMalformed),
        }
    }
}

fn message_stream(
    stream: impl Stream<Item = Vec<u8>>,
    log_target: &'static str
) -> impl Stream<Item = ConnectionMessage<Vec<u8>>> {
    stream.map(move |payload| {
        let message = ConnectionMessage::parse(&payload);
        debug!("{log_target}: Received message: {message:?}");
        message
    })
}

fn message_sink<S: Sink<Vec<u8>>>(
    sink: S,
    log_target: &'static str
) -> impl Sink<ConnectionMessage<Vec<u8>>, Error = S::Error> {
    // sink.with(|message: ConnectionMessage<Vec<u8>>| async { Ok(message.format()) });
    let sink = Box::pin(sink);
    futures::sink::unfold(sink, move |mut sink, message: ConnectionMessage<Vec<u8>>| {
        async move {
            debug!("{log_target}: Sending payload: {message:?}");
            if let Some(message) = message.format() {
                sink.send(message).await?;
            }
            Ok(sink)
        }
    })
}