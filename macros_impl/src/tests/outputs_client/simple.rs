trait TodoServer {
    type Error: ::core::error::Error;
    async fn get_todos(&self) -> Result<Vec<Todo>, Self::Error>;
    async fn get_todo(&self, name: String) -> Result<Option<Todo>, Self::Error>;
    async fn new_todo(&self, todo: Todo) -> Result<(), Self::Error>;
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
    NewTodo(()),
}

struct TodoServerClient<T: ::trait_link::Transport>(T);

impl<T: ::trait_link::Transport> TodoServerClient<T> {
    fn new(transport: T) -> Self {
        Self(transport)
    }
}

impl<T: ::trait_link::Transport> TodoServer for TodoServerClient<T> {
    type Error = ::trait_link::LinkError<<T as ::trait_link::Transport>::Error>;
    async fn get_todos(&self) -> Result<Vec<Todo>, ::trait_link::LinkError> {
        if let TodoServerResponse::GetTodos(value) =
            self.0.send(TodoServerRequest::GetTodos()).await?
        {
            Ok(value)
        } else {
            Err(::trait_link::LinkError::WrongResponseType)
        }
    }
    async fn get_todo(&self, name: String) -> Result<Option<Todo>, ::trait_link::LinkError> {
        if let TodoServerResponse::GetTodo(value) =
            self.0.send(TodoServerRequest::GetTodo(name)).await?
        {
            Ok(value)
        } else {
            Err(::trait_link::LinkError::WrongResponseType)
        }
    }
    async fn new_todo(&self, todo: Todo) -> Result<(), ::trait_link::LinkError> {
        if let TodoServerResponse::NewTodo(value) =
            self.0.send(TodoServerRequest::NewTodo(todo)).await?
        {
            Ok(value)
        } else {
            Err(::trait_link::LinkError::WrongResponseType)
        }
    }
}
