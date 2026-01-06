use crate::format::{Cbor, DynFormat, Format, Json};
use crate::{Handler, Rpc};
use axum::body::Bytes;
use axum::extract::{FromRequest, Request};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderName, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use bon::__::IsUnset;
use bon::Builder;
use futures::FutureExt;
use futures::future::BoxFuture;
use std::convert::Infallible;
use std::ops::Deref;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::Service;

/// A service which serves an RPC service in multiple formats as part of an axum server
#[derive(Builder)]
pub struct Axum<H>
where
    H: Handler + Send + Sync + 'static,
{
    #[builder(field)]
    methods: Vec<Method>,
    #[builder(field)]
    formats: Formats<H::Rpc>,
    #[builder(setters(name = arc_service, vis = "pub(crate)"))]
    handler: Arc<H>,
}

impl<H> Clone for Axum<H>
where
    H: Handler + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            methods: self.methods.clone(),
            formats: self.formats.clone(),
            handler: self.handler.clone(),
        }
    }
}

impl<H, State> AxumBuilder<H, State>
where
    H: Handler + Send + Sync + 'static,
    State: axum_builder::State,
{
    /// the server handler to server requests with, Handler should be implemented for `&S`
    pub fn handler(self, service: H) -> AxumBuilder<H, axum_builder::SetHandler<State>>
    where
        State::Handler: IsUnset,
    {
        self.arc_service(Arc::new(service))
    }

    /// Add a format to support
    pub fn format(mut self, format: &'static impl for<'a> Format<RpcRequest<H>, RpcResponse<H>>) -> Self {
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
        Json: for<'a> Format<RpcRequest<H>, RpcResponse<H>>,
    {
        self.format(&Json)
    }

    /// Add CBOR support to this server
    #[cfg(feature = "cbor")]
    pub fn allow_cbor(self) -> Self
    where
        Cbor: for<'a> Format<RpcRequest<H>, RpcResponse<H>>,
    {
        self.format(&Cbor)
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

type Formats<R> = Vec<&'static dyn DynFormat<<R as Rpc>::Request, <R as Rpc>::Response>>;
type RpcRequest<H> = <HandlerRpc<H> as Rpc>::Request;
type RpcResponse<H> = <HandlerRpc<H> as Rpc>::Response;
type HandlerRpc<H> = <H as Handler>::Rpc;

type Success = (StatusCode, [(HeaderName, &'static str); 1], Vec<u8>);

impl<H> Service<Request> for Axum<H>
where
    H: Handler + Send + Sync + 'static,
{
    type Response = Result<Success, Error>;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        Box::pin(self.call_internal(req).map(Ok))
    }
}

impl<H> Axum<H>
where
    H: Handler + Send + Sync + 'static,
{
    fn call_internal(
        &mut self,
        req: Request,
    ) -> impl Future<Output = Result<Success, Error>> + Send + 'static {
        let methods = self.methods.clone();
        let formats = self.formats.clone();
        let server = self.handler.clone();
        async move {
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
                .find(|format| format.info().http_content_type == content_type)
                .ok_or(Error::UnsupportedContentType)?;
            let bytes = Bytes::from_request(req, &())
                .await
                .map_err(|error| Error::Internal(error.to_string()))?;
            let request = format
                .read(&mut bytes.deref())
                .map_err(Error::Deserialise)?;
            let response = server.deref().handle(request).await;
            let mut buffer = Vec::new();
            format
                .write(response, &mut buffer)
                .map_err(Error::Serialise)?;
            Ok((
                StatusCode::OK,
                [(CONTENT_TYPE, format.info().http_content_type)],
                buffer,
            ))
        }
    }
}

/// An Error which may occur when handling RPC requests
pub enum Error {
    /// The wrong HTTP method was used
    WrongMethod,
    /// There was no Content-Type Header
    NoContentType,
    /// The given Content-Type is nopt supported
    UnsupportedContentType,
    /// An Error occurred while deserialising the request
    Deserialise(String),
    /// An Error occurred while serialising the response
    Serialise(String),
    /// An internal error occurred while processing the request
    Internal(String),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Error::WrongMethod => (
                StatusCode::NOT_FOUND,
                "No resource found with the provided method".to_string(),
            )
                .into_response(),
            Error::NoContentType => (
                StatusCode::BAD_REQUEST,
                "No Content-Type Header provided".to_string(),
            )
                .into_response(),
            Error::UnsupportedContentType => (
                StatusCode::BAD_REQUEST,
                "provided Content-Type not supported".to_string(),
            )
                .into_response(),
            Error::Deserialise(error) => (
                StatusCode::BAD_REQUEST,
                format!("Could not parse request: {error}"),
            )
                .into_response(),
            Error::Serialise(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Could not serialise response: {error}"),
            )
                .into_response(),
            Error::Internal(error) => (StatusCode::INTERNAL_SERVER_ERROR, error).into_response(),
        }
    }
}
