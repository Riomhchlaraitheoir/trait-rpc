#[allow(
    unused_imports,
    reason = "These might not always be used, but they should be available in this module anyway"
)]
pub use todo_service::{
    TodoService, TodoServiceAsyncClient, TodoServiceBlockingClient, TodoServiceServer,
};

#[allow(
    unused_imports,
    reason = "These might not always be used, but it's easier to include always"
)]
mod todo_service {
    use super::*;
    use std::convert::Infallible;
    use std::marker::PhantomData;
    use ::trait_rpc::{
        client::{AsyncClient, BlockingClient, MappedClient, StreamClient, WrongResponseType},
        futures::sink::{Sink, SinkExt},
        futures::stream::{Stream, StreamExt},
        serde::{Deserialize, Serialize},
        server::{Handler, IntoHandler},
        Rpc, RpcWithServer
    };

    /// A service for managing to-do items
    ///
    /// This is the [Rpc](::trait_rpc::Rpc) definition for this service
    pub struct TodoService;

    impl Rpc for TodoService {
        type AsyncClient<_Client: AsyncClient<Self::Request, Self::Response>> =
            TodoServiceAsyncClient<_Client>;
        type BlockingClient<_Client: BlockingClient<Self::Request, Self::Response>> =
            TodoServiceBlockingClient<_Client>;
        type Request = Request;
        type Response = Response;
        fn async_client<_Client: AsyncClient<Request, Response>>(
            transport: _Client,
        ) -> TodoServiceAsyncClient<_Client> {
            TodoServiceAsyncClient(transport)
        }
        fn blocking_client<_Client: BlockingClient<Request, Response>>(
            transport: _Client,
        ) -> TodoServiceBlockingClient<_Client> {
            TodoServiceBlockingClient(transport)
        }
    }

    impl<Server: TodoServiceServer> RpcWithServer<Server> for TodoService {
        type Handler = TodoServiceHandler<Server>;
        fn handler(server: Server) -> Self::Handler {
            TodoServiceHandler(server)
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(crate = "::trait_rpc::serde")]
    #[serde(tag = "method", content = "args")]
    pub enum Request {
        #[serde(rename = "get_todos")]
        GetTodos(),
        #[serde(rename = "get_todo")]
        GetTodo(String),
        #[serde(rename = "new_todo")]
        NewTodo(Todo),
    }
    impl ::trait_rpc::Request for Request {
        fn is_streaming_response(&self) -> bool {
            match self {
                Self::GetTodos(..) => false,
                Self::GetTodo(..) => false,
                Self::NewTodo(..) => false,
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(crate = "::trait_rpc::serde")]
    #[serde(tag = "method", content = "result")]
    pub enum Response {
        #[serde(rename = "get_todos")]
        GetTodos(Vec<Todo>),
        #[serde(rename = "get_todo")]
        GetTodo(Option<Todo>),
        #[serde(rename = "new_todo")]
        NewTodo(()),
    }
    impl Response {
        fn fn_name(&self) -> &'static str {
            match self {
                Self::GetTodos(..) => "get_todos",
                Self::GetTodo(..) => "get_todo",
                Self::NewTodo(..) => "new_todo",
            }
        }
    }

    /// A service for managing to-do items
    ///
    /// This is the trait which is used by the server side in order to serve the client
    pub trait TodoServiceServer: Send + Sync {
        /// Get a list of to-do items
        fn get_todos(&self) -> impl Future<Output = Vec<Todo>> + Send;
        /// Get a to-do item by name, returns None if no to-do item with the given name exists
        fn get_todo(&self, name: String) -> impl Future<Output = Option<Todo>> + Send;
        /// Create a new to-do item
        fn new_todo(&self, todo: Todo) -> impl Future<Output = ()> + Send;
    }

    /// A [Handler](Handler) which handles requests/responses for a given service
    #[derive(Debug, Clone)]
    pub struct TodoServiceHandler<_Server>(_Server);

    impl<_Server: TodoServiceServer> Handler for TodoServiceHandler<_Server> {
        type Rpc = TodoService;
        async fn handle(&self, request: Request) -> Response {
            match request {
                Request::GetTodos() => Response::GetTodos(self.0.get_todos().await),
                Request::GetTodo(name) => Response::GetTodo(self.0.get_todo(name).await),
                Request::NewTodo(todo) => Response::NewTodo(self.0.new_todo(todo).await),
                _ => panic!("This is a streaming method, must call handle_streaming"),
            }
        }
        async fn handle_stream_response<S: Sink<Response, Error = Infallible> + Send + 'static>(
            &self,
            request: Request,
            sink: S,
        ) {
            match request {
                _ => panic!("This is not a streaming method, must call handle"),
            }
        }
    }

    /// A service for managing to-do items
    ///
    /// This is the async client for the service, it produces requests from method calls
    /// (including chained method calls) and sends the requests with the given
    /// [transport](::trait_rpc::AsyncClient) before returning the response
    ///
    /// The return value is always wrapped in a result: `Result<T, _Client::Error>` where `T` is the service return value
    #[derive(Debug, Copy, Clone)]
    pub struct TodoServiceAsyncClient<_Client>(_Client);

    #[allow(clippy::future_not_send)]

    impl<_Client: AsyncClient<Request, Response>> TodoServiceAsyncClient<_Client> {
        /// Get a list of to-do items
        pub async fn get_todos(&self) -> Result<Vec<Todo>, _Client::Error> {
            match self.0.send(Request::GetTodos()).await? {
                Response::GetTodos(value) => Ok(value),
                other => Err(WrongResponseType::new("get_todos", other.fn_name()).into()),
            }
        }
        /// Get a to-do item by name, returns None if no to-do item with the given name exists
        pub async fn get_todo(&self, name: String) -> Result<Option<Todo>, _Client::Error> {
            match self.0.send(Request::GetTodo(name)).await? {
                Response::GetTodo(value) => Ok(value),
                other => Err(WrongResponseType::new("get_todo", other.fn_name()).into()),
            }
        }
        /// Create a new to-do item
        pub async fn new_todo(&self, todo: Todo) -> Result<(), _Client::Error> {
            match self.0.send(Request::NewTodo(todo)).await? {
                Response::NewTodo(value) => Ok(value),
                other => Err(WrongResponseType::new("new_todo", other.fn_name()).into()),
            }
        }
    }

    /// A service for managing to-do items
    ///
    /// This is the blocking client for the service, it produces requests from method calls
    /// (including chained method calls) and sends the requests with the given
    /// [transport](::trait_rpc::AsyncClient) before returning the response
    ///
    /// The return value is always wrapped in a result: `Result<T, _Client::Error>` where `T` is the service return value
    #[derive(Debug, Copy, Clone)]
    pub struct TodoServiceBlockingClient<_Client>(_Client);

    impl<_Client: BlockingClient<Request, Response>> TodoServiceBlockingClient<_Client> {
        /// Get a list of to-do items
        pub fn get_todos(&self) -> Result<Vec<Todo>, _Client::Error> {
            match self.0.send(Request::GetTodos())? {
                Response::GetTodos(value) => Ok(value),
                other => Err(WrongResponseType::new("get_todos", other.fn_name()).into()),
            }
        }
        /// Get a to-do item by name, returns None if no to-do item with the given name exists
        pub fn get_todo(&self, name: String) -> Result<Option<Todo>, _Client::Error> {
            match self.0.send(Request::GetTodo(name))? {
                Response::GetTodo(value) => Ok(value),
                other => Err(WrongResponseType::new("get_todo", other.fn_name()).into()),
            }
        }
        /// Create a new to-do item
        pub fn new_todo(&self, todo: Todo) -> Result<(), _Client::Error> {
            match self.0.send(Request::NewTodo(todo))? {
                Response::NewTodo(value) => Ok(value),
                other => Err(WrongResponseType::new("new_todo", other.fn_name()).into()),
            }
        }
    }
}
