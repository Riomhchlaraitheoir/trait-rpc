use axum::Router;
use std::collections::HashMap;
use tokio::sync::RwLock;
use trait_link::server::axum::Axum;

include!("traits/resources.rs");

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route_service("/api/books", 
               Axum::builder()
                   .handler(Resources::server(ResourceServer::<Book>::default()))
                   .allow_json()
                   .allow_cbor()
                   .allow_post()
                   .build()
        
        )
        .route_service("/api/authors",
               Axum::builder()
                   .handler(Resources::server(ResourceServer::<Author>::default()))
                   .allow_json()
                   .allow_cbor()
                   .allow_post()
                   .build()
        
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap()
}

struct ResourceServer<T> {
    map: RwLock<HashMap<u64, T>>,
}

impl<T> Default for ResourceServer<T> {
    fn default() -> Self {
        Self {
            map: RwLock::default(),
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
