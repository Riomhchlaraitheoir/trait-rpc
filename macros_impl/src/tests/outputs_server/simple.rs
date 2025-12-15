
trait TodoServer {
    fn get_todos(&self) -> impl ::core::future::Future<Output = Vec<Todo>> + Send;
    fn get_todo(&self, name: String) -> impl ::core::future::Future<Output = Option<Todo>> + Send;
    fn new_todo(&self, todo: Todo) -> impl ::core::future::Future<Output = ()> + Send;
}

#[derive(Debug, ::trait_link::serde::Serialize, ::trait_link::serde::Deserialize)]
enum TodoServerRequest {
    GetTodos(),
    GetTodo(String),
    NewTodo(Todo),
}

#[derive(Debug, ::trait_link::serde::Serialize, ::trait_link::serde::Deserialize)]
enum TodoServerResponse {
    GetTodos(Vec<Todo>),
    GetTodo(Option<Todo>),
    NewTodo(())
}

struct TodoServerRpcServer<T: TodoServer>(T);

impl<T: TodoServer + Sync> ::trait_link::Rpc for TodoServerRpcServer<T> {
    type Request = TodoServerRequest;
    type Response = TodoServerResponse;

    async fn process(&self, request: Self::Request) -> Self::Response {
        #[allow(clippy::unit_arg)]
        match request {
            TodoServerRequest::GetTodos() => {
                TodoServerResponse::GetTodos(self.0.get_todos().await)
            }
            TodoServerRequest::GetTodo(name) => {
                TodoServerResponse::GetTodo(self.0.get_todo(name).await)
            }
            TodoServerRequest::NewTodo(todo) => {
                TodoServerResponse::NewTodo(self.0.new_todo(todo).await)
            }
        }
    }
}
