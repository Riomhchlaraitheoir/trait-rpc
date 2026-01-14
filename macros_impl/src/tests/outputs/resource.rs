#[allow(unused_imports, reason = "These might not always be used, but they should be available in this module anyway")]
pub use resources::{Resources, ResourcesAsyncClient, ResourcesBlockingClient, ResourcesServer};
#[allow(unused_imports, reason = "These might not always be used, but it's easier to include always")]
mod resources {
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
    /// This is the [Rpc](::trait_rpc::Rpc) definition for this service
    pub struct Resources<T>(PhantomData<fn() -> (T,)>);
    impl<T> Rpc for Resources<T>
    where
        T: Send + 'static,
    {
        type AsyncClient<_Client: AsyncClient<Self::Request, Self::Response>> =
            ResourcesAsyncClient<_Client, T>;
        type BlockingClient<_Client: BlockingClient<Self::Request, Self::Response>> =
            ResourcesBlockingClient<_Client, T>;
        type Request = Request<T>;
        type Response = Response<T>;
        fn async_client<_Client: AsyncClient<Request<T>, Response<T>>>(
            transport: _Client,
        ) -> ResourcesAsyncClient<_Client, T> {
            ResourcesAsyncClient(transport, PhantomData::<fn() -> (T,)>)
        }
        fn blocking_client<_Client: BlockingClient<Request<T>, Response<T>>>(
            transport: _Client,
        ) -> ResourcesBlockingClient<_Client, T> {
            ResourcesBlockingClient(transport, PhantomData::<fn() -> (T,)>)
        }
    }

    impl<Server: ResourcesServer<T>, T: Send + 'static> RpcWithServer<Server> for Resources<T> {
        type Handler = ResourcesHandler<Server, T>;
        fn handler(server: Server) -> Self::Handler {
            ResourcesHandler(server, PhantomData::<fn() -> (T,)>)
        }
    }
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(crate = "::trait_rpc::serde")]
    #[serde(tag = "method", content = "args")]
    pub enum Request<T> {
        #[serde(rename = "subscribe")]
        Subscribe(),
        #[serde(rename = "list")]
        List(),
        #[serde(rename = "get")]
        Get(u64),
        #[serde(rename = "new")]
        New(T),
    }
    impl<T> ::trait_rpc::Request for Request<T> {
        fn is_streaming_response(&self) -> bool {
            match self {
                Self::Subscribe(..) => true,
                Self::List(..) => false,
                Self::Get(..) => false,
                Self::New(..) => false,
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(crate = "::trait_rpc::serde")]
    #[serde(tag = "method", content = "result")]
    pub enum Response<T> {
        #[serde(rename = "subscribe")]
        Subscribe(T),
        #[serde(rename = "list")]
        List(Vec<T>),
        #[serde(rename = "get")]
        Get(Option<T>),
        #[serde(rename = "new")]
        New(()),
    }
    impl<T> Response<T> {
        fn fn_name(&self) -> &'static str {
            match self {
                Self::Subscribe(..) => "subscribe",
                Self::List(..) => "list",
                Self::Get(..) => "get",
                Self::New(..) => "new",
            }
        }
    }
    /// This is the trait which is used by the server side in order to serve the client
    pub trait ResourcesServer<T>: Send + Sync {
        fn subscribe(&self, sink: impl Sink<T, Error = Infallible> + Send + 'static) -> impl Future<Output = ()> + Send;
        fn list(&self) -> impl Future<Output = Vec<T>> + Send;
        fn get(&self, id: u64) -> impl Future<Output = Option<T>> + Send;
        fn new(&self, value: T) -> impl Future<Output = ()> + Send;
    }
    /// A [Handler](Handler) which handles requests/responses for a given service
    #[derive(Debug, Clone)]
    pub struct ResourcesHandler<_Server, T>(_Server, (PhantomData<fn() -> (T,)>));
    impl<_Server: ResourcesServer<T>, T: Send + 'static> Handler for ResourcesHandler<_Server, T> {
        type Rpc = Resources<T>;
        async fn handle(&self, request: Request<T>) -> Response<T> {
            match request {
                Request::List() => Response::List(self.0.list().await),
                Request::Get(id) => Response::Get(self.0.get(id).await),
                Request::New(value) => Response::New(self.0.new(value).await),
                _ => panic!("This is a streaming method, must call handle_streaming"),
            }
        }
        async fn handle_stream_response<S: Sink<Response<T>, Error = Infallible> + Send + 'static>(&self, request: Request<T>, sink: S) {
            match request {
                Request::Subscribe() => {
                    let sink = sink.with(async |value| Result::<_, S::Error>::Ok(Response::Subscribe(value)));
                    self.0.subscribe(sink).await;
                },
                _ => panic!("This is not a streaming method, must call handle"),
            }
        }
    }

    /// This is the async client for the service, it produces requests from method calls
    /// (including chained method calls) and sends the requests with the given
    /// [transport](::trait_rpc::AsyncClient) before returning the response
    ///
    /// The return value is always wrapped in a result: `Result<T, _Client::Error>` where `T` is the service return value
    #[derive(Debug, Copy, Clone)]
    pub struct ResourcesAsyncClient<_Client, T>(_Client, (PhantomData<fn() -> (T,)>));
    #[allow(clippy::future_not_send)]
         impl<_Client: AsyncClient<Request<T>, Response<T>>, T> ResourcesAsyncClient<_Client, T> {
        pub async fn subscribe(&self) -> Result<impl Stream<Item = Result<T, _Client::Error>>, _Client::Error> where _Client: StreamClient<Request<T>, Response<T>> {
            let stream = self.0.send_streaming_response(Request::Subscribe()).await?;
            Ok(stream.map(|value| {
                match value {
                    Ok(Response::Subscribe(value)) => Ok(value),
                    Ok(other) => Err(WrongResponseType::new("subscribe", other.fn_name()).into()),
                    Err(error) => {Err(error.into())},
                }
            }))
        }
        pub async fn list(&self) -> Result<Vec<T>, _Client::Error> {
            match self.0.send(Request::List()).await? {
                Response::List(value) => Ok(value),
                other => Err(WrongResponseType::new("list", other.fn_name()).into()),
            }
        }
        pub async fn get(&self, id: u64) -> Result<Option<T>, _Client::Error> {
            match self.0.send(Request::Get(id)).await? {
                Response::Get(value) => Ok(value),
                other => Err(WrongResponseType::new("get", other.fn_name()).into()),
            }
        }
        pub async fn new(&self, value: T) -> Result<(), _Client::Error> {
            match self.0.send(Request::New(value)).await? {
                Response::New(value) => Ok(value),
                other => Err(WrongResponseType::new("new", other.fn_name()).into()),
            }
        }
    }
    /// This is the blocking client for the service, it produces requests from method calls
    /// (including chained method calls) and sends the requests with the given
    /// [transport](::trait_rpc::AsyncClient) before returning the response
    ///
    /// The return value is always wrapped in a result: `Result<T, _Client::Error>` where `T` is the service return value
    #[derive(Debug, Copy, Clone)]
    pub struct ResourcesBlockingClient<_Client, T>(_Client, (PhantomData<fn() -> (T,)>));
    impl<_Client: BlockingClient<Request<T>, Response<T>>, T> ResourcesBlockingClient<_Client, T> {
        pub fn list(&self) -> Result<Vec<T>, _Client::Error> {
            match self.0.send(Request::List())? {
                Response::List(value) => Ok(value),
                other => Err(WrongResponseType::new("list", other.fn_name()).into()),
            }
        }
        pub fn get(&self, id: u64) -> Result<Option<T>, _Client::Error> {
            match self.0.send(Request::Get(id))? {
                Response::Get(value) => Ok(value),
                other => Err(WrongResponseType::new("get", other.fn_name()).into()),
            }
        }
        pub fn new(&self, value: T) -> Result<(), _Client::Error> {
            match self.0.send(Request::New(value))? {
                Response::New(value) => Ok(value),
                other => Err(WrongResponseType::new("new", other.fn_name()).into()),
            }
        }
    }
}
