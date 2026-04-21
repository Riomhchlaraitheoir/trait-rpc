use async_executor::Executor;
use axum::extract::{FromRequestParts, State};
use futures::{Sink, SinkExt, StreamExt};
use macros::rpc;
use std::marker::PhantomData;
use std::pin::pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::{RwLock, oneshot};
use tokio::time::{sleep, timeout};
use tracing::log::Level;
use tracing::{Instrument, debug, info, info_span};
use trait_rpc::client::SimpleClient;
use trait_rpc::client::websocket::new_websocket_transport;
use trait_rpc::format::json::Json;
use trait_rpc::server::StreamError;
use trait_rpc::server::axum::Axum;
use trait_rpc::stream::client::StreamClient;
use trait_rpc::{Rpc, client};

#[rpc]
trait Service {
    fn simple_get(&self) -> String;
    fn simple_set(&self, value: u64);
    fn stream(&self) -> Stream<u64>;
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

    async fn stream<'a>(&'a self, sink: impl Sink<u64, Error = StreamError> + Send + 'a) {
        let mut sink = pin!(sink);
        for i in 0..10000 {
            debug!("Sending {i}");
            sink.send(i).await.expect("failed to send value");
        }
    }
}

async fn run_server(
    state: Arc<RwLock<ServerState>>,
    shutdown_signal: impl Future<Output = ()> + Send + 'static,
) {
    let server = axum::Router::new().route_service(
        "/",
        Axum::builder()
            .rpc(PhantomData::<Service>)
            .server(PhantomData::<ServiceImpl>)
            .state(state)
            .allow_json()
            .allow_cbor()
            .build(),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:9865")
        .await
        .unwrap();
    info!("Starting server");
    axum::serve::serve(listener, server)
        .with_graceful_shutdown(shutdown_signal)
        .await
        .unwrap();
}

type Client = <Service as Rpc>::AsyncClient<SimpleClient<Json, StreamClient>>;

#[tokio::test]
async fn websocket_test() {
    use futures::FutureExt;
    simple_logger::init_with_level(Level::Info).unwrap();
    println!("Starting test");

    let (stop_server, shutdown_signal) = oneshot::channel::<()>();
    let state = Arc::<RwLock<ServerState>>::default();
    let server = spawn(
        run_server(state.clone(), shutdown_signal.into_future().map(|_| ()))
            .instrument(info_span!("server")),
    );
    info!("Server running");
    sleep(Duration::from_secs(1)).await;
    let client = client::builder()
        .non_blocking()
        .transport(
            new_websocket_transport("ws://localhost:9865/", Json)
                .await
                .expect("Failed to create transport"),
        )
        .format(&Json)
        .build();
    let client = Service::async_client(client);

    let executor = Executor::new();
    let tests = [
        executor.spawn(test_simple_get(&client)),
        executor.spawn(test_simple_set(&client, &state)),
        executor.spawn(test_stream(&client)),
    ];
    let tests = tests
        .into_iter()
        .map(|test| timeout(Duration::from_secs(10), test));
    executor
        .run(async {
            let mut pass = true;
            for (i, test) in tests.into_iter().enumerate() {
                if test
                    .instrument(info_span!("client", test = i))
                    .await
                    .is_ok()
                {
                    info!("test {i} complete");
                } else {
                    pass = false;
                    info!("test {i} timed out");
                }
            }
            assert!(pass, "Tests did not complete successfully");
        })
        .await;
    info!("Sending shutdown signal to server");
    stop_server.send(()).unwrap();
    server.await.unwrap();
}

async fn test_simple_get(client: &Client) {
    let response = client.simple_get().await;
    let response = response.expect("Failed to get response");
    assert_eq!(&response, "hello world");
}

async fn test_simple_set(client: &Client, state: &Arc<RwLock<ServerState>>) {
    client.simple_set(42).await.expect("Failed to set value");
    assert_eq!(state.read().await.set_value, 42);
}

async fn test_stream(client: &Client) {
    let mut stream = client.stream().await.expect("Failed to set value");
    for i in 0..10000 {
        let value = stream
            .next()
            .await
            .expect("stream closed early")
            .expect("failed to get value");
        assert_eq!(i, value);
        info!("Received {i}");
    }
}
