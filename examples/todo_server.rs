#![doc = include_str!("./examples.md")]

use axum::body::Bytes;
use axum::extract::{FromRequestParts, State, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum_extra::headers::authorization::Bearer;
use axum_extra::{headers, TypedHeader};
use derive_more::Deref;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use trait_rpc::format::cbor::Cbor;
use trait_rpc::format::json::Json;
use trait_rpc::format::Format;
use trait_rpc::server::axum::{handle_request, handle_websocket, HandleError, WebsocketError};
use trait_rpc::stream::server::ConnectionHooks;

include!("traits/todo.rs");

#[derive(Default, Clone)]
struct ServerState {
    todos: Arc<RwLock<Vec<Todo>>>,
}

#[derive(Deref, FromRequestParts)]
struct Todos {
    #[from_request(via(State))]
    #[deref]
    state: ServerState,
    auth: TypedHeader<headers::Authorization<Bearer>>,
}

impl TodoServiceServer for Todos {
    async fn get_todos(&self) -> Vec<Todo> {
        self.todos.read().await.deref().clone()
    }

    async fn get_todo(&self, name: String) -> Option<Todo> {
        self.todos
            .read()
            .await
            .iter()
            .find(|todo| todo.name == name)
            .cloned()
    }

    async fn new_todo(&self, todo: Todo) {
        if self.auth.token() == "valid" {
            self.todos.write().await.push(todo);
        }
    }
}

impl ConnectionHooks<TodoService> for Todos {
    async fn await_next<T>(&self, request: impl Future<Output = Option<T>> + Send) -> Option<T> {
        // 10 minutes idle timeout
        tokio::time::timeout(Duration::from_mins(10), request).await.ok().unwrap_or_default()
    }
}

#[tokio::main]
async fn main() {
    let app = axum::Router::new()
        .route("/api/todo", get(service_websocket))
        .route("/api/todo", post(handle_service_request))
        .with_state(ServerState::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve::serve(listener, app).await.unwrap();
}

async fn service_websocket(conn: WebSocketUpgrade, server: Todos) -> impl IntoResponse {
    let result =
        handle_websocket::<TodoService, _>(conn, server, &[&Json as &dyn Format<_, _>, &Cbor]);
    match result {
        Ok(response) => response,
        Err(WebsocketError::UnsupportedFormat) => StatusCode::BAD_REQUEST.into_response(),
    }
}

async fn handle_service_request(
    server: Todos,
    headers: HeaderMap,
    bytes: Bytes,
) -> impl IntoResponse {
    let result = handle_request::<TodoService, _>(
        server,
        headers,
        bytes,
        &[&Json as &dyn Format<_, _>, &Cbor],
    )
    .await;
    match result {
        Ok(response) => response,
        Err(
            HandleError::UnsupportedContentType(_)
            | HandleError::NoContentType
            | HandleError::InvalidContentType(_),
        ) => StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response(),
        Err(HandleError::Deserialise(_)) => StatusCode::BAD_REQUEST.into_response(),
        Err(HandleError::Serialise(_)) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
