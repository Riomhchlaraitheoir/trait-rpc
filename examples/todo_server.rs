#![doc = include_str!("./examples.md")]

use axum::extract::{FromRequestParts, State};
use axum_extra::headers::authorization::Bearer;
use axum_extra::{headers, TypedHeader};
use derive_more::Deref;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::RwLock;
use trait_rpc::server::axum::Axum;

include!("traits/todo.rs");

#[derive(Default, Clone)]
struct ServerState {
    todos: Arc<RwLock<Vec<Todo>>>
}

#[derive(Deref, FromRequestParts)]
struct Todos {
    #[deref]
    state: State<ServerState>,
    auth: TypedHeader<headers::Authorization<Bearer>>,
}

impl TodoServiceServer for Todos {
    async fn get_todos(&self) -> Vec<Todo> {
        self.todos.read().await.deref().clone()
    }

    async fn get_todo(&self, name: String) -> Option<Todo> {
        self.todos.read().await.iter().find(|todo| todo.name == name).cloned()
    }

    async fn new_todo(&self, todo: Todo) {
        if self.auth.token() == "valid" {
            self.todos.write().await.push(todo);
        }
    }
}

#[tokio::main]
async fn main() {
    let app = axum::Router::new()
        .route_service("/api/todo",
               Axum::builder()
                   .rpc(PhantomData::<TodoService>)
                   .server(PhantomData::<Todos>)
                   .state(ServerState::default())
                   .allow_json()
                   .allow_post()
                   .allow_put()
                   .build()
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve::serve(listener, app).await.unwrap();
}