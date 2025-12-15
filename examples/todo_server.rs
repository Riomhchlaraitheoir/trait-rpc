use std::sync::{Arc, PoisonError, RwLock};
use axum::routing::post;
use trait_link::serde::{Serialize, Deserialize};
use trait_link::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Todo {
    title: String,
    description: String,
    done: bool,
    deadline: chrono::NaiveDateTime,
}

trait TodoServer {
    fn get_todos(&self) -> impl Future<Output = Vec<Todo>> + Send;
    fn get_todo(&self, name: String) -> impl Future<Output = Option<Todo>> + Send;
    fn new_todo(&self, todo: Todo) -> impl Future<Output = ()> + Send;
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
enum TodoServerRequest {
    GetTodos(),
    GetTodo(String),
    NewTodo(Todo),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
enum TodoServerResponse {
    GetTodos(Vec<Todo>),
    GetTodo(Option<Todo>),
    NewTodo(())
}

struct TodoServerRpcServer<T>(T);

impl<T: TodoServer + Sync> Rpc for TodoServerRpcServer<T> {
    type Request = TodoServerRequest;
    type Response = TodoServerResponse;

    async fn process(&self, request: Self::Request) -> Self::Response {
        match request {
            TodoServerRequest::GetTodos() => {
                TodoServerResponse::GetTodos(self.0.get_todos().await)
            }
            TodoServerRequest::GetTodo(name) => {
                TodoServerResponse::GetTodo(self.0.get_todo(name).await)
            }
            TodoServerRequest::NewTodo(todo) => {
                #[allow(clippy::unit_arg)]
                TodoServerResponse::NewTodo(self.0.new_todo(todo).await)
            }
        }
    }
}

struct Todos {
    todos: RwLock<Vec<Todo>>
}

impl TodoServer for Todos {
    async fn get_todos(&self) -> Vec<Todo> {
        self.todos.read().unwrap_or_else(PoisonError::into_inner).clone()
    }

    async fn get_todo(&self, name: String) -> Option<Todo> {
        self.todos.read().unwrap_or_else(PoisonError::into_inner).iter().find(|todo| todo.title == name).cloned()
    }

    async fn new_todo(&self, todo: Todo) -> () {
        self.todos.write().unwrap_or_else(PoisonError::into_inner).push(todo)
    }
}

#[tokio::main]
async fn main() {
    let state = Arc::new(TodoServerRpcServer(Todos {
        todos: Default::default(),
    }));
    let app = axum::Router::new()
        .route("/api/todo", post(trait_link::server::serve::<TodoServerRpcServer<Todos>>))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve::serve(listener, app).await.unwrap()
}
