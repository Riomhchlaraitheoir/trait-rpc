#![doc = include_str!("./examples.md")]

use axum::Router;
use std::collections::HashMap;
use std::convert::Infallible;
use std::pin::pin;
use futures::{Sink, SinkExt};
use tokio::sync::{broadcast, RwLock};
use tokio::sync::broadcast::error::RecvError;
use trait_rpc::RpcWithServer;
use trait_rpc::server::axum::Axum;

include!("traits/resources.rs");

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route_service("/api/books", 
               Axum::builder()
                   .handler(Resources::handler(ResourceServer::<Book>::default()))
                   .allow_json()
                   .allow_cbor()
                   .allow_post()
                   .build()
        
        )
        .route_service("/api/authors",
               Axum::builder()
                   .handler(Resources::handler(ResourceServer::<Author>::default()))
                   .allow_json()
                   .allow_cbor()
                   .allow_post()
                   .build()
        
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

struct ResourceServer<T> {
    map: RwLock<HashMap<u64, T>>,
    new: broadcast::Sender<T>,
}

impl<T: Resource> Default for ResourceServer<T> {
    fn default() -> Self {
        Self {
            map: RwLock::default(),
            new: broadcast::channel(10).0
        }
    }
}

trait Resource: Clone + Send + Sync + 'static {
    fn id(&self) -> u64;
}

impl Resource for Book {
    fn id(&self) -> u64 {
        self.id
    }
}

impl Resource for Author {
    fn id(&self) -> u64 {
        self.id
    }
}

impl<T: Resource> ResourcesServer<T> for ResourceServer<T> {
    async fn subscribe(&self, sink: impl Sink<T, Error = Infallible> + Send + 'static) {
        let mut sink = pin!(sink);
        let mut receiver = self.new.subscribe();
        loop {
            match receiver.recv().await {
                Ok(value) => match sink.send(value).await {
                    Ok(()) => {},
                    Err(err) => {
                        match err {}
                    }
                },
                Err(RecvError::Closed) => break,
                Err(RecvError::Lagged(_)) => {},
            }
        }
    }

    async fn list(&self) -> Vec<T> {
        self.map.read().await.values().cloned().collect()
    }

    async fn get(&self, id: u64) -> Option<T> {
        self.map.read().await.get(&id).cloned()
    }

    async fn new(&self, value: T) {
        self.map.write().await.insert(value.id(), value);
    }
}
