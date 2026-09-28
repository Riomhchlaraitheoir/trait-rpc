use async_executor::Executor;
use axum::body::Bytes;
use axum::extract::{FromRequestParts, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use macros::rpc;
use std::sync::Arc;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::log;
use trait_rpc::client::reqwest::Reqwest;
use trait_rpc::client::SimpleClient;
use trait_rpc::format::cbor::Cbor;
use trait_rpc::format::json::Json;
use trait_rpc::format::Format;
use trait_rpc::server::axum::{handle_request, HandleError};
use trait_rpc::{client, Rpc};

#[rpc]
trait Service {
    fn simple_get() -> String;
    fn simple_set(value: u64);
}

#[derive(Default)]
struct ServerState {
    set_value: u64,
}

#[derive(FromRequestParts)]
struct ServiceImpl {
    state: State<Arc<RwLock<ServerState>>>,
}

impl ServiceServer for ServiceImpl {
    async fn simple_get(&self) -> String {
        "hello world".to_owned()
    }

    async fn simple_set(&self, value: u64) {
        self.state.write().await.set_value = value;
    }
}

async fn run_server(state: Arc<RwLock<ServerState>>) {
    let server = axum::Router::new()
        .route("/", post(handle))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:7456")
        .await
        .unwrap();
    axum::serve::serve(listener, server).await.unwrap();
}

async fn handle(server: ServiceImpl, headers: HeaderMap, bytes: Bytes) -> impl IntoResponse {
    let result =
        handle_request::<Service, _>(server, headers, bytes, &[&Json as &dyn Format<_, _>, &Cbor])
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

type Client = <Service as Rpc>::AsyncClient<SimpleClient<Json, Reqwest>>;

#[tokio::test]
async fn http_test() {
    simple_logger::SimpleLogger::new().with_level(log::LevelFilter::Debug).init().unwrap();
    let state = Arc::<RwLock<ServerState>>::default();
    let server = spawn(run_server(state.clone()));
    sleep(Duration::from_secs(1)).await;
    let client = client::builder()
        .non_blocking()
        .transport(Reqwest::builder().url("http://localhost:7456").build())
        .format(&Json)
        .build();
    let client = Arc::new(Service::async_client(client));

    let executor = Executor::new();
    let tasks = [
        executor.spawn(test_simple_get(client.clone())),
        executor.spawn(test_simple_set(client.clone(), &state)),
    ];
    executor.run(futures::future::join_all(tasks)).await;
    server.abort();
}

async fn test_simple_get(client: Arc<Client>) {
    let response = client.simple_get().await;
    let response = response.expect("Failed to get response");
    assert_eq!(&response, "hello world");
}

async fn test_simple_set(client: Arc<Client>, state: &Arc<RwLock<ServerState>>) {
    client.simple_set(42).await.expect("Failed to set value");
    assert_eq!(state.read().await.set_value, 42);
}
