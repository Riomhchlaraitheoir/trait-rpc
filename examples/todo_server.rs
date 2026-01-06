use std::ops::Deref;
use tokio::sync::RwLock;
use trait_link::server::axum::Axum;

include!("traits/todo.rs");

#[derive(Default)]
struct Todos {
    todos: RwLock<Vec<Todo>>
}

impl TodoServiceServer for Todos {
    async fn get_todos(&self) -> Vec<Todo> {
        self.todos.read().await.deref().clone()
    }

    async fn get_todo(&self, name: String) -> Option<Todo> {
        self.todos.read().await.iter().find(|todo| todo.name == name).cloned()
    }

    async fn new_todo(&self, todo: Todo) -> () {
        self.todos.write().await.push(todo)
    }
}

#[tokio::main]
async fn main() {
    let app = axum::Router::new()
        .route_service("/api/todo",
               Axum::builder()
                   .handler(TodoService::server(Todos::default()))
                   .allow_json()
                   .allow_post()
                   .allow_put()
                   .build()
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve::serve(listener, app).await.unwrap()
}