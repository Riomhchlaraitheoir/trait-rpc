#[allow(unused_imports, reason = "only used if certain features are enabled")]
use crate::format;
use crate::format::{Format, IsFormat};
use crate::{Handler, Rpc, get_request_id, prepend_id};
use axum::RequestExt;
use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{ConnectInfo, FromRequest, FromRequestParts, Request, WebSocketUpgrade};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use bon::__::IsUnset;
use bon::Builder;
use futures::FutureExt;
use futures::future::BoxFuture;
use std::convert::Infallible;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::task::{Context, Poll};
use tower::Service;
use tracing::{Instrument, debug, info, info_span};
use crate::server::axum::axum_builder::{SetRpc, SetServer};
use crate::server::IntoHandler;

/// A service which serves an RPC service in multiple formats as part of an axum server
#[derive(Builder)]
pub struct Axum<R, Server, State>
where
    R: Rpc + 'static,
    Server: FromRequestParts<State> + IntoHandler<R> + 'static,
    State: Clone + Send + Sync + 'static,
    <Server as IntoHandler<R>>::Handler: Sync + 'static
{
    #[builder(field)]
    methods: Vec<Method>,
    #[builder(field)]
    formats: Formats<R>,
    #[builder(setters(name = rpc_type, vis = "pub(crate)"))]
    rpc: PhantomData<fn() -> R>,
    #[builder(setters(name = server_type, vis = "pub(crate)"))]
    server: PhantomData<fn() -> Server>,
    state: State,
    #[builder(default)]
    enable_websockets: bool,
}

impl<R, Server, State> Clone for Axum<R, Server, State>
where
    R: Rpc + 'static,
    Server: FromRequestParts<State> + IntoHandler<R> + 'static,
    State: Clone + Send + Sync + 'static,
    <Server as IntoHandler<R>>::Handler: Sync + 'static
{
    fn clone(&self) -> Self {
        Self {
            methods: self.methods.clone(),
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
    Server: FromRequestParts<State> + IntoHandler<R> + 'static,
    State: Clone + Send + Sync + 'static,
    <Server as IntoHandler<R>>::Handler: Sync + 'static
{
    /// Define the Rpc type
    ///
    /// This method exits so that the generic arg can be defined without having to define the other args
    pub fn rpc(self, _: PhantomData<R>) -> AxumBuilder<R, Server, State, SetRpc<BuildState>>
    where BuildState::Rpc: IsUnset
    {
        self.rpc_type(PhantomData)
    }

    /// Define the Server type
    ///
    /// This method exits so that the generic arg can be defined without having to define the other args
    pub fn server(self, _: PhantomData<Server>) -> AxumBuilder<R, Server, State, SetServer<BuildState>>
    where BuildState::Server: IsUnset
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

    /// Add a method to allow, NOTE: method must allow a body in both request and response
    pub fn method(mut self, method: Method) -> Self {
        self.methods.push(method);
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

    /// Allow POST requests
    pub fn allow_post(self) -> Self {
        self.method(Method::POST)
    }

    /// Allow PUT requests
    pub fn allow_put(self) -> Self {
        self.method(Method::PUT)
    }

    /// Allow PATCH requests
    pub fn allow_patch(self) -> Self {
        self.method(Method::PATCH)
    }
}

type Formats<R> = Vec<&'static dyn Format<<R as Rpc>::Request, <R as Rpc>::Response>>;
type RpcFormat<H> = &'static dyn Format<RpcRequest<H>, RpcResponse<H>>;
type RpcRequest<R> = <R as Rpc>::Request;
type RpcResponse<R> = <R as Rpc>::Response;

impl<R, Server, State> Service<Request> for Axum<R, Server, State>
where
    R: Rpc + 'static,
    Server: FromRequestParts<State> + IntoHandler<R> + 'static,
    State: Clone + Send + Sync + 'static,
    <Server as IntoHandler<R>>::Handler: Sync + 'static
{
    type Response = Result<Response, Error<<Server as FromRequestParts<State>>::Rejection>>;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        Box::pin(self.call_internal(req).map(Ok))
    }
}

impl<R, Server, State> Axum<R, Server, State>
where
    R: Rpc + 'static,
    Server: FromRequestParts<State> + IntoHandler<R> + 'static,
    State: Clone + Send + Sync + 'static,
    <Server as IntoHandler<R>>::Handler: Sync + 'static
{
    fn call_internal(
        &self,
        mut req: Request,
    ) -> impl Future<Output = Result<Response, Error<<Server as FromRequestParts<State>>::Rejection>>> + Send + 'static {
        let methods = self.methods.clone();
        let formats = self.formats.clone();
        let state = self.state.clone();
        async move {
            let server: Server = req.extract_parts_with_state(&state).await.map_err(Error::LoadServer)?;
            let handler = server.into_handler();
            if let Ok(mut ws) = req.extract_parts::<WebSocketUpgrade>().await
                && let Ok(ConnectInfo(addr)) = req.extract_parts::<ConnectInfo<SocketAddr>>().await
            {
                println!("Upgrading to websocket at {addr}");
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
                        info_span!(target: "websocket", "Websocket connection", address = addr.to_string())
                    )
                ));
            }
            if !methods.contains(req.method()) {
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

    async fn handle_websocket(
        mut socket: WebSocket,
        format: &'static dyn Format<RpcRequest<R>, RpcResponse<R>>,
        handler: <Server as IntoHandler<R>>::Handler,
    ) {
        info!("Started websocket connection");
        if socket
            .send(Message::Ping(Bytes::from_static(&[1, 2, 3])))
            .await
            .is_err()
        {
            debug!("Failed to send ping message");
            return;
        }
        debug!("Sent ping message");

        loop {
            let Some(msg) = socket.recv().await else {
                info!("Websocket disconnected abruptly");
                return;
            };
            let msg = match msg {
                Ok(msg) => msg,
                Err(error) => {
                    info!("Websocket disconnected with error: {error}");
                    return;
                }
            };
            let response = match msg {
                Message::Text(_) => Some(Message::Text("text frames not supported".into())),
                Message::Binary(bytes) => {
                    Some(match Self::handle_request(format, &bytes, &handler).await {
                        Ok(msg) => Message::Binary(msg.into()),
                        Err(error) => Message::Text(error.into()),
                    })
                }
                Message::Ping(bytes) => Some(Message::Pong(bytes)),
                Message::Pong(_) => None,
                Message::Close(frame) => {
                    if let Some(frame) = frame {
                        info!(
                            "Websocket connection closed, code: {}, reason: {}",
                            frame.code, frame.reason
                        );
                    } else {
                        info!("Websocket connection closed without frame");
                    }
                    Some(Message::Close(None))
                }
            };
            if let Some(response) = response
                && socket.send(response).await.is_err()
            {
                debug!("Failed to send response message");
                return;
            }
        }
    }

    async fn handle_request(
        format: RpcFormat<R>,
        request: &[u8],
        handler: &<Server as IntoHandler<R>>::Handler,
    ) -> Result<Vec<u8>, String> {
        let (request_id, request) = get_request_id(request);
        let request: RpcRequest<R> = format
            .read(request)
            .map_err(|error| format!("Failed to parse request: {error}"))?;
        let response = handler.handle(request).await;
        let response = format
            .write(response)
            .map_err(|error| format!("Failed to write response: {error}"))?;
        let response = prepend_id(request_id, response);
        Ok(response)
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
