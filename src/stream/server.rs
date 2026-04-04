//! Helpers for dealing with stream connections on the server side

use crate::client::HandleError;
use crate::format::Format;
use crate::server::StreamError;
use crate::stream::{message_sink, message_stream, ConnectionError, ConnectionMessage};
use crate::{Handler, Request, Rpc};
use async_executor::Executor;
use futures::channel::mpsc;
use futures::channel::mpsc::{unbounded, SendError};
use futures::future::{select, Either};
use futures::{Sink, SinkExt, Stream, StreamExt};
use std::fmt::Debug;
use std::future::ready;
use std::pin::{pin, Pin};
use thiserror::Error;
use tracing::{debug, error, info_span, warn, Instrument};

/// Handle incoming requests, responding appropriately
///
/// # Errors
/// Returns an error when `sink.send(_)` returns an error or if an internal error occurs
/// If an error is returned, an error will have already been sent to the client where possible
pub async fn serve_request_stream<In, Out, H>(
    stream: In,
    sink: Out,
    handler: H,
    format: &'static (dyn Format<<H::Rpc as Rpc>::Request, <H::Rpc as Rpc>::Response> + 'static),
) -> Result<(), ServerError<Out::Error>>
where
    H: Handler + Sync,
    In: Stream<Item = Vec<u8>> + Send,
    Out: Sink<Vec<u8>, Error: Send> + Send,
    <H::Rpc as Rpc>::Response: Send + Sync,
    <H::Rpc as Rpc>::Request: Send,
{
    let stream = message_stream(stream, "server");
    let stream = formatted_stream(stream, format);
    let sink = message_sink(sink, "server");
    let sink = formatted_sink(sink, format);
    handle_formatted_requests(stream, sink, handler).await
}

/// An error that can occur while handling stream request
#[derive(Debug, Error)]
pub enum ServerError<E> {
    /// Sink failed to send item
    #[error(transparent)]
    Sink(#[from] E),
    /// The response channel which processes responses has closed unexpectedly
    #[error("Response channel closed unexpectedly")]
    ResponseChannelClosed,
}

async fn handle_formatted_requests<In, Out, H>(
    stream: In,
    sink: Out,
    handler: H,
) -> Result<(), ServerError<Out::Error>>
where
    H: Handler + Sync,
    In: Stream<Item = ConnectionMessage<<H::Rpc as Rpc>::Request>>,
    Out: Sink<ConnectionMessage<<H::Rpc as Rpc>::Response>, Error: Send> + Send,
    <H::Rpc as Rpc>::Response: Send,
    <H::Rpc as Rpc>::Request: Send,
{
    let (sender, receiver) = unbounded();
    let receiver = pin!(receiver);
    let sink = pin!(sink);
    let executor = Executor::new();
    executor.run(async {
        let request_handler = request_handler(stream, &handler, sender, &executor).instrument(info_span!("request_handler"));
        let request_handler = pin!(request_handler);
        let response_handler = response_handler(receiver, sink).instrument(info_span!("response_handler"));
        let response_handler = pin!(response_handler);

        match select(request_handler, response_handler).await {
            Either::Left(((), _)) => Ok(()),
            Either::Right((result, _)) => result
        }
    }).instrument(info_span!("server executor")).await
}

async fn response_handler<Response: Debug, Out>(
    mut receiver: Pin<&mut mpsc::UnboundedReceiver<ConnectionMessage<Response>>>,
    mut sink: Pin<&mut Out>,
) -> Result<(), ServerError<Out::Error>>
where
    Out: Sink<ConnectionMessage<Response>>,
{
    loop {
        debug!("Waiting for next response");
        let response = receiver.next().await;
        let Some(message) = response else {
            error!("response channel closed unexpectedly");
            sink.send(ConnectionMessage::ConnectionError(
                ConnectionError::ResponseChannelClosed,
            ))
            .await?;
            return Err(ServerError::ResponseChannelClosed);
        };
        sink.send(message).await?;
    }
}

async fn request_handler<'a, H, In>(
    stream: In,
    handler: &'a H,
    mut sender: mpsc::UnboundedSender<ConnectionMessage<<H::Rpc as Rpc>::Response>>,
    executor: &Executor<'a>,
) where
    H: Handler + Sync,
    In: Stream<Item = ConnectionMessage<<H::Rpc as Rpc>::Request>>,
    <H::Rpc as Rpc>::Response: Send,
    <H::Rpc as Rpc>::Request: Send,
{
    let mut stream = pin!(stream);
    while let Some(message) = stream.next().await {
        let (request_id, request) = match message.payload() {
            Ok(request) => request,
            Err(message) => {
                if let Some(conn_error) = message.connection_error() {
                    warn!("connection error from client: {conn_error}");
                } else {
                    let _ = sender.send(message).await;
                }
                continue;
            }
        };

        debug!("Handling request {request_id:?}: {request:?}");
        let handler_span = info_span!("handler", request_id, method = request.name().as_ref());
        if request.is_streaming_response() {
            let sink = sender.clone();
            let sink = sink.with(move |response| {
                debug!("Sending stream response for request {request_id:?}: {response:?}");
                ready(Ok(ConnectionMessage::Payload {
                    request_id,
                    payload: response,
                }))
            });
            let sink = sink.sink_map_err(|send_error: SendError| {
                if send_error.is_full() {
                    StreamError::SendFailed("internal channel is full".to_string())
                } else if send_error.is_disconnected() {
                    StreamError::Closed
                } else {
                    panic!("Unknown failure")
                }
            });
            let mut sender = sender.clone();
            executor.spawn(async move {
                handler.handle_stream_response(request, sink).await;
                sender.send(ConnectionMessage::StreamEnd { request_id }).await
            }.instrument(handler_span)).detach();
        } else {
            let mut sender = sender.clone();
            executor.spawn(async move {
                let response = handler.handle(request).await;
                sender
                    .send(ConnectionMessage::Payload {
                        request_id,
                        payload: response,
                    })
                    .await
                    .expect("response channel closed");
            }.instrument(handler_span)).detach();
        }
    }
}

fn formatted_stream<Read, Write>(
    stream: impl Stream<Item = ConnectionMessage<Vec<u8>>>,
    format: &'static (dyn Format<Read, Write> + 'static),
) -> impl Stream<Item = ConnectionMessage<Read>> {
    stream.map(
        move |message| match message.map(|payload| format.read(&payload)) {
            Ok(message) => message,
            Err((request_id, error)) => ConnectionMessage::HandleError {
                request_id,
                error: HandleError::BadRequest(format!("failed to parse request: {error}")),
            },
        },
    )
}

fn formatted_sink<Read, Write, S: Sink<ConnectionMessage<Vec<u8>>>>(
    sink: S,
    format: &'static (dyn Format<Read, Write> + 'static),
) -> impl Sink<ConnectionMessage<Write>, Error = S::Error> {
    sink.with(
        async |message: ConnectionMessage<Write>| -> Result<ConnectionMessage<Vec<u8>>, S::Error> {
            Ok(match message.map(|payload| format.write(payload)) {
                Ok(message) => message,
                Err((request_id, error)) => ConnectionMessage::HandleError {
                    request_id,
                    error: HandleError::BadRequest(format!("failed to parse request: {error}")),
                },
            })
        },
    )
}

#[cfg(test)]
mod test {
    use crate::server::StreamError;
    use crate::stream::server::handle_formatted_requests;
    use crate::stream::ConnectionMessage;
    use crate::RpcWithServer;
    use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
    use futures::{Sink, SinkExt, Stream, StreamExt};
    use macros::rpc;
    use std::pin::{pin, Pin};
    use std::task::{Context, Poll};
    use std::time::Duration;
    use pin_project::pin_project;
    use tokio::time::sleep;
    use tracing::{debug, info};
    use crate::stream::server::test::test_rpc::{Request, Response};

    const FIBONACCI_LIMIT: u32 = 1000;

    #[rpc(trait_rpc = crate)]
    trait TestRpc {
        fn simple_call(&self, id: u32) -> String;
        fn fibonacci(&self) -> Stream<u32>;
    }

    impl PartialEq for test_rpc::Response {
        fn eq(&self, other: &Self) -> bool {
            match (self, other) {
                (Self::SimpleCall(a), Self::SimpleCall(b)) => a == b,
                (Self::Fibonacci(a), Self::Fibonacci(b)) => a == b,
                _ => false,
            }
        }
    }

    struct TestImpl;

    impl TestRpcServer for TestImpl {
        async fn simple_call(&self, id: u32) -> String {
            println!("received simple call request (id: {id})");
            sleep(Duration::from_millis(100)).await;
            "value".to_string()
        }

        async fn fibonacci<'a>(&'a self, sink: impl Sink<u32, Error = StreamError> + Send + 'a) {
            let mut sink = pin!(sink);
            sink.send(0).await.expect("send failed");
            let mut current = 1;
            let mut previous = 0;
            while current < FIBONACCI_LIMIT {
                debug!("sending fibonacci: {current}");
                sink.send(current).await.expect("send failed");
                let next = current + previous;
                (previous, current) = (current, next);
                sleep(Duration::from_millis(10)).await;
            }
            debug!("finished sending fibonacci");
        }
    }

    #[pin_project]
    struct SinkAndStream {
        #[pin]
        sink: UnboundedSender<ConnectionMessage<Response>>,
        #[pin]
        stream: UnboundedReceiver<ConnectionMessage<Request>>
    }

    impl Stream for SinkAndStream {
        type Item = ConnectionMessage<Request>;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            self.project().stream.poll_next(cx)
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            self.stream.size_hint()
        }
    }

    impl Sink<ConnectionMessage<Response>> for SinkAndStream {
        type Error = <UnboundedSender<ConnectionMessage<Response>> as Sink<ConnectionMessage<Response>>>::Error;

        fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.project().sink.poll_ready(cx)
        }

        fn start_send(self: Pin<&mut Self>, item: ConnectionMessage<Response>) -> Result<(), Self::Error> {
            self.project().sink.start_send(item)
        }

        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.project().sink.poll_flush(cx)
        }

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.project().sink.poll_close(cx)
        }
    }

    #[tokio::test]
    async fn simple_call() {
        let (mut request_sender, request_receiver) = unbounded();
        let (response_sender, mut response_receiver) = unbounded();
        tokio::spawn(async {
            println!("Starting test server");
            let handler = TestRpc::handler(TestImpl);
            handle_formatted_requests(
                request_receiver,
                response_sender,
                handler,
            )
            .await
            .expect("server error");
            println!("Finished test server");
        });

        sleep(Duration::from_millis(100)).await;
        request_sender
            .send(ConnectionMessage::Payload {
                request_id: 0,
                payload: test_rpc::Request::SimpleCall(1),
            })
            .await
            .expect("failed to send request");
        sleep(Duration::from_millis(100)).await;
        let response = response_receiver
            .next()
            .await
            .expect("failed to receive response");
        let ConnectionMessage::Payload {
            request_id: 0,
            payload: response,
        } = response
        else {
            panic!("unexpected response message: {response:?}");
        };
        assert_eq!(
            response,
            test_rpc::Response::SimpleCall("value".to_string())
        );
    }

    #[tokio::test]
    async fn fibonacci() {
        let (mut request_sender, request_receiver) = unbounded();
        let (response_sender, mut response_receiver) = unbounded();
        // Some streams may be tied together in this fashion and split using [StreamExt::split]
        //
        // This can cause problems, since the sink and stream will not actually be independent,
        // but rather tied together with a `futures::lock::BiLock`
        //
        // For example, if one future is waiting on the stream using `stream.next().await`, it
        // will hold the lock until it receives something. That means that another future trying
        // to send will also have to wait until something is received, this is a problem
        //
        // This test uses a split stream in order to ensure that the server implementation doesn't
        // cause this deadlock bug to rear its ugly head
        let merged_stream = SinkAndStream {
            sink: response_sender,
            stream: request_receiver,
        };
        let (sink, stream) = merged_stream.split();
        tokio::spawn(async {
            println!("Starting test server");
            let handler = TestRpc::handler(TestImpl);
            handle_formatted_requests(
                stream,
                sink,
                handler,
            )
            .await
            .expect("server error");
            println!("Finished test server");
        });

        sleep(Duration::from_millis(100)).await;
        request_sender
            .send(ConnectionMessage::Payload {
                request_id: 123,
                payload: test_rpc::Request::Fibonacci(),
            })
            .await
            .expect("failed to send request");
        sleep(Duration::from_millis(100)).await;
        let mut next = async || -> u32 {
            debug!("Awaiting next value");
            let message = response_receiver
                .next()
                .await
                .expect("response channel closed");
            let ConnectionMessage::Payload {
                request_id: 123,
                payload: response,
            } = message
            else {
                panic!("unexpected response message: {message:?}");
            };
            let test_rpc::Response::Fibonacci(number) = response else {
                panic!("unexpected response type: {response:?}");
            };
            info!(target: "test", "Received: {number}");
            number
        };
        let mut previous = next().await;
        println!("fib: {previous}");
        let mut current = next().await;
        println!("fib: {current}");
        loop {
            let expected = previous + current;
            if expected > FIBONACCI_LIMIT {
                break;
            }
            (previous, current) = (current, next().await);
            assert_eq!(expected, current);
            println!("fib: {current}");
        }
        let message = response_receiver
            .next()
            .await
            .expect("response channel closed");

        assert!(matches!(message, ConnectionMessage::StreamEnd { request_id: 123 }), "unexpected message: {message:?}");
        info!("test finished");
    }
}
