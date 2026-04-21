#[allow(unused_imports, reason = "only used if certain features are enabled")]
use crate::format;
use crate::format::Format;
use crate::server::axum::axum_builder::{SetEnableWebsockets, SetRpc, SetServer};
use crate::server::{IntoHandler, StreamError};
use crate::{Handler, Rpc};
use axum::RequestExt;
use axum::body::Bytes;
use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use bon::__::IsUnset;
use bon::Builder;
use futures::FutureExt;
use futures::future::BoxFuture;
use std::convert::Infallible;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::task::{Context, Poll};
use tower::Service;
#[allow(
    unused_imports,
    reason = "may be unused depending on features, not worth splitting behind toggles"
)]
use tracing::{Instrument, debug, error, info, info_span, warn};
#[cfg(feature = "websocket-server")]
use {
    crate::{format::IsFormat, stream::server::serve_request_stream},
    axum::extract::{
        ConnectInfo, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    futures::{
        Sink, Stream, StreamExt,
        future::{Either, ready},
        lock::BiLock,
        stream::once,
    },
    std::{net::SocketAddr, pin::pin},
};

/// A service which serves an RPC service in multiple formats as part of an axum server
#[derive(Builder)]
pub struct Axum<R, Server, State>
where
    R: Rpc + 'static,
    Server: FromRequestParts<State> + IntoHandler<R> + 'static,
    State: Clone + Send + Sync + 'static,
    <Server as IntoHandler<R>>::Handler: Sync + 'static,
    <R as Rpc>::Response: Send + Sync,
    <R as Rpc>::Request: Send,
{
    #[builder(field)]
    formats: Formats<R>,
    #[builder(setters(name = rpc_type, vis = "pub(crate)"))]
    rpc: PhantomData<fn() -> R>,
    #[builder(setters(name = server_type, vis = "pub(crate)"))]
    server: PhantomData<fn() -> Server>,
    state: State,
    #[builder(default, setters(vis = "", name = enable_ws))]
    enable_websockets: bool,
}

impl<R, Server, State> Clone for Axum<R, Server, State>
where
    R: Rpc + 'static,
    Server: FromRequestParts<State> + IntoHandler<R> + 'static,
    State: Clone + Send + Sync + 'static,
    <Server as IntoHandler<R>>::Handler: Sync + 'static,
    <R as Rpc>::Response: Send + Sync,
    <R as Rpc>::Request: Send,
{
    fn clone(&self) -> Self {
        Self {
            formats: self.formats.clone(),
            rpc: PhantomData,
            server: PhantomData,
            state: self.state.clone(),
            enable_websockets: self.enable_websockets,
        }
    }
}

impl<R, Server, State, BuildState> AxumBuilder<R, Server, State, BuildState>
where
    BuildState: axum_builder::State,
    R: Rpc + 'static,
    Server: FromRequestParts<State, Rejection: Debug> + IntoHandler<R> + 'static,
    State: Clone + Send + Sync + 'static,
    <Server as IntoHandler<R>>::Handler: Sync + 'static,
    <R as Rpc>::Response: Send + Sync,
    <R as Rpc>::Request: Send,
{
    /// Define the Rpc type
    ///
    /// This method exits so that the generic arg can be defined without having to define the other args
    pub fn rpc(self, _: PhantomData<R>) -> AxumBuilder<R, Server, State, SetRpc<BuildState>>
    where
        BuildState::Rpc: IsUnset,
    {
        self.rpc_type(PhantomData)
    }

    /// Define the Server type
    ///
    /// This method exits so that the generic arg can be defined without having to define the other args
    pub fn server(
        self,
        _: PhantomData<Server>,
    ) -> AxumBuilder<R, Server, State, SetServer<BuildState>>
    where
        BuildState::Server: IsUnset,
    {
        self.server_type(PhantomData)
    }

    /// Add a format to support
    pub fn format(
        mut self,
        format: &'static impl for<'a> Format<RpcRequest<R>, RpcResponse<R>>,
    ) -> Self {
        self.formats.push(format);
        self
    }

    /// Add JSON support to this server
    #[cfg(feature = "json")]
    pub fn allow_json(self) -> Self
    where
        format::json::Json: for<'a> Format<RpcRequest<R>, RpcResponse<R>>,
    {
        self.format(&format::json::Json)
    }

    /// Add CBOR support to this server
    #[cfg(feature = "cbor")]
    pub fn allow_cbor(self) -> Self
    where
        format::cbor::Cbor: for<'a> Format<RpcRequest<R>, RpcResponse<R>>,
    {
        self.format(&format::cbor::Cbor)
    }

    /// Enable Websocket support
    pub fn enable_websockets(self) -> AxumBuilder<R, Server, State, SetEnableWebsockets<BuildState>>
    where
        BuildState::EnableWebsockets: IsUnset,
    {
        const {
            assert!(
                cfg!(feature = "websocket-server"),
                "Websockets require feature `websocket-server`"
            );
        };
        self.enable_ws(true)
    }
}

type Formats<R> = Vec<&'static dyn Format<<R as Rpc>::Request, <R as Rpc>::Response>>;
#[cfg(feature = "websocket-server")]
type RpcFormat<H> = &'static dyn Format<RpcRequest<H>, RpcResponse<H>>;
type RpcRequest<R> = <R as Rpc>::Request;
type RpcResponse<R> = <R as Rpc>::Response;

impl<R, Server, State> Service<Request> for Axum<R, Server, State>
where
    R: Rpc + 'static,
    Server: FromRequestParts<State, Rejection: Debug> + IntoHandler<R> + 'static,
    State: Clone + Send + Sync + 'static,
    <Server as IntoHandler<R>>::Handler: Sync + 'static,
    <R as Rpc>::Response: Send + Sync,
    <R as Rpc>::Request: Send,
{
    type Response = Result<Response, Error<<Server as FromRequestParts<State>>::Rejection>>;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        Box::pin(
            self.call_internal(req)
                .map(Ok)
                .instrument(info_span!("server", service = R::service_name())),
        )
    }
}

impl<R, Server, State> Axum<R, Server, State>
where
    R: Rpc + 'static,
    Server: FromRequestParts<State, Rejection: Debug> + IntoHandler<R> + 'static,
    State: Clone + Send + Sync + 'static,
    <Server as IntoHandler<R>>::Handler: Sync + 'static,
    <R as Rpc>::Response: Send + Sync,
    <R as Rpc>::Request: Send,
{
    fn call_internal(
        &self,
        mut req: Request,
    ) -> impl Future<
        Output = Result<Response, Error<<Server as FromRequestParts<State>>::Rejection>>,
    > + Send
    + 'static {
        let formats = self.formats.clone();
        let state = self.state.clone();
        #[cfg(feature = "websocket-server")]
        let websockets_enabled = self.enable_websockets;
        async move {
            let server: Server = req.extract_parts_with_state(&state).await.map_err(|err| {
                info!("Failed to load service for request: {:?}", err);
                Error::LoadServer(err)
            })?;
            let handler = server.into_handler();
            #[cfg(feature = "websocket-server")]
            if websockets_enabled && let Ok(mut ws) = req.extract_parts::<WebSocketUpgrade>().await
            {
                debug!("received websocket upgrade request");
                let addr = req.extract_parts::<ConnectInfo<SocketAddr>>().await.ok();
                let addr = addr.map_or_else(
                    || "<address not loaded>".to_string(),
                    |addr| addr.to_string(),
                );
                info!("Upgrading to websocket at {addr}");
                let protocols: Vec<_> = formats
                    .iter()
                    .copied()
                    .map(IsFormat::content_type)
                    .collect();
                ws = ws.protocols(protocols.clone());
                let protocol = ws
                    .selected_protocol()
                    .ok_or_else(|| Error::UnsupportedSubprotocol(protocols.clone()))?;
                let format = formats
                    .iter()
                    .find(|format| format.content_type() == protocol)
                    .ok_or(Error::UnsupportedSubprotocol(protocols))?;
                let format: RpcFormat<R> = *format;
                return Ok(ws.on_upgrade(move |socket|
                    Self::handle_websocket(socket, format, handler).instrument(
                        info_span!(target: "websocket", "Websocket connection", address = addr.clone())
                    )
                ));
            }
            if req.method() != Method::POST {
                debug!("wrong method, expecting POST, but got {}", req.method());
                return Err(Error::WrongMethod);
            }
            let content_type = req
                .headers()
                .get(CONTENT_TYPE)
                .ok_or(Error::NoContentType)?;
            let content_type = content_type
                .to_str()
                .map_err(|error| Error::Internal(error.to_string()))?;
            let content_type = content_type.split(';').next().unwrap_or(content_type);
            let format = formats
                .iter()
                .find(|format| format.content_type() == content_type)
                .ok_or(Error::UnsupportedContentType)?;
            let bytes = Bytes::from_request(req, &())
                .await
                .map_err(|error| Error::Internal(error.to_string()))?;
            let request = format
                .read(&bytes)
                .map_err(|error| Error::Deserialise(error.to_string()))?;
            let response = handler.handle(request).await;
            let response = format
                .write(response)
                .map_err(|error| Error::Serialise(error.to_string()))?;
            Ok((
                StatusCode::OK,
                [(CONTENT_TYPE, format.content_type())],
                response,
            )
                .into_response())
        }
    }
}

#[cfg(feature = "websocket-server")]
impl<R, Server, State> Axum<R, Server, State>
where
    R: Rpc + 'static,
    Server: FromRequestParts<State> + IntoHandler<R> + 'static,
    State: Clone + Send + Sync + 'static,
    <Server as IntoHandler<R>>::Handler: Sync + 'static,
    <R as Rpc>::Response: Send + Sync,
    <R as Rpc>::Request: Send,
{
    async fn handle_websocket(
        websocket: WebSocket,
        format: &'static dyn Format<RpcRequest<R>, RpcResponse<R>>,
        handler: <Server as IntoHandler<R>>::Handler,
    ) {
        let (sink_lock, stream_lock) = BiLock::new(websocket);
        let stream = Self::websocket_stream(stream_lock);
        let sink = Self::websocket_sink(sink_lock);
        if let Err(error) = serve_request_stream(stream, sink, handler, format).await {
            error!("Error occurred on websocket connection: {error}");
        }
    }

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
}

impl From<axum::Error> for StreamError {
    fn from(error: axum::Error) -> Self {
        Self::SendFailed(error.to_string())
    }
}

/// An Error which may occur when handling RPC requests
pub enum Error<Server> {
    /// The wrong HTTP method was used
    WrongMethod,
    /// There was no Content-Type Header
    NoContentType,
    /// The given Content-Type is not supported
    UnsupportedContentType,
    /// The given Sec-WebSocket-Protocol is not supported
    UnsupportedSubprotocol(Vec<&'static str>),
    /// An Error occurred while deserialising the request
    Deserialise(String),
    /// An Error occurred while serialising the response
    Serialise(String),
    /// An internal error occurred while processing the request
    Internal(String),
    /// A rejection when getting the server from the request
    LoadServer(Server),
}

impl<Server: IntoResponse> IntoResponse for Error<Server> {
    fn into_response(self) -> Response {
        match self {
            Self::WrongMethod => (
                StatusCode::NOT_FOUND,
                "No resource found with the provided method".to_string(),
            )
                .into_response(),
            Self::NoContentType => (
                StatusCode::BAD_REQUEST,
                "No Content-Type Header provided".to_string(),
            )
                .into_response(),
            Self::UnsupportedContentType => (
                StatusCode::BAD_REQUEST,
                "provided Content-Type not supported".to_string(),
            )
                .into_response(),
            Self::UnsupportedSubprotocol(subprotocols) => (
                StatusCode::BAD_REQUEST,
                format!(
                    "provided subprotocol is not supported, supported subprotocols: {}",
                    subprotocols.join(", ")
                ),
            )
                .into_response(),
            Self::Deserialise(error) => (
                StatusCode::BAD_REQUEST,
                format!("Could not parse request: {error}"),
            )
                .into_response(),
            Self::Serialise(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Could not serialise response: {error}"),
            )
                .into_response(),
            Self::Internal(error) => (StatusCode::INTERNAL_SERVER_ERROR, error).into_response(),
            Self::LoadServer(error) => error.into_response(),
        }
    }
}
