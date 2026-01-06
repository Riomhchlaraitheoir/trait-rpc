#[allow(unused_imports, reason = "These might not always be used, but they should be available in this module anyway")]
pub use api_service::{
    ApiService, ApiServiceAsyncClient, ApiServiceBlockingClient, ApiServiceServer,
};

#[allow(unused_imports, reason = "These might not always be used, but it's easier to include always")]
mod api_service {
    use super::*;
    use ::trait_link::{
        Rpc,
        client::{AsyncClient, BlockingClient, MappedClient, WrongResponseType},
        serde::{Deserialize, Serialize},
        server::Handler,
    };
    use std::marker::PhantomData;
    /// This is the [Rpc](::trait_link::Rpc) definition for this service
    pub struct ApiService;
    impl Rpc for ApiService {
        type AsyncClient<_Client: AsyncClient<Self::Request, Self::Response>> =
            ApiServiceAsyncClient<_Client>;
        type BlockingClient<_Client: BlockingClient<Self::Request, Self::Response>> =
            ApiServiceBlockingClient<_Client>;
        type Request = Request;
        type Response = Response;
        fn async_client<_Client: AsyncClient<Request, Response>>(
            transport: _Client,
        ) -> ApiServiceAsyncClient<_Client> {
            ApiServiceAsyncClient(transport)
        }
        fn blocking_client<_Client: BlockingClient<Request, Response>>(
            transport: _Client,
        ) -> ApiServiceBlockingClient<_Client> {
            ApiServiceBlockingClient(transport)
        }
    }
    impl ApiService {
        /// Create a new [Handler](trait_link::Handler) for the service
        pub fn server(server: impl ApiServiceServer) -> impl Handler<Rpc = Self> {
            ApiServiceHandler(server)
        }
    }
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(crate = "::trait_link::serde")]
    #[serde(tag = "method", content = "args")]
    pub enum Request {
        #[serde(rename = "users")]
        Users(<UsersService as Rpc>::Request),
        #[serde(rename = "login")]
        Login(String, String),
    }
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(crate = "::trait_link::serde")]
    #[serde(tag = "method", content = "result")]
    pub enum Response {
        #[serde(rename = "users")]
        Users(<UsersService as Rpc>::Response),
        #[serde(rename = "login")]
        Login(Option<LoginToken>),
    }
    impl Response {
        fn fn_name(&self) -> &'static str {
            match self {
                Self::Users(..) => "users",
                Self::Login(..) => "login",
            }
        }
    }

    /// This is the trait which is used by the server side in order to serve the client
    pub trait ApiServiceServer: Send + Sync {
        fn users(&self) -> impl Future<Output = impl Handler<Rpc = UsersService>> + Send;
        fn login(
            &self,
            username: String,
            password: String,
        ) -> impl Future<Output = Option<LoginToken>> + Send;
    }
    /// A [Handler](Handler) which handles requests/responses for a given service
    #[derive(Debug, Clone)]
    pub struct ApiServiceHandler<_Server>(_Server);
    impl<_Server: ApiServiceServer> Handler for ApiServiceHandler<_Server> {
        type Rpc = ApiService;
        async fn handle(&self, request: Request) -> Response {
            match request {
                Request::Users(request) => {
                    let response = self.0.users().await.handle(request).await;
                    Response::Users(response)
                }
                Request::Login(username, password) => {
                    Response::Login(self.0.login(username, password).await)
                }
            }
        }
    }



    /// This is the async client for the service, it produces requests from method calls
    /// (including chained method calls) and sends the requests with the given
    /// [transport](::trait_link::AsyncClient) before returning the response
    ///
    /// The return value is always wrapped in a result: `Result<T, _Client::Error>` where `T` is the service return value
    #[derive(Debug, Copy, Clone)]
    pub struct ApiServiceAsyncClient<_Client>(_Client);
    impl<_Client: AsyncClient<Request, Response>> ApiServiceAsyncClient<_Client> {
        pub fn users(
            &self,
        ) -> <UsersService as Rpc>::AsyncClient<
            MappedClient<
                _Client,
                <UsersService as Rpc>::Request,
                Request,
                <UsersService as Rpc>::Response,
                Response,
                (),
            >,
        > {
            UsersService::async_client(MappedClient::new(
                self.0.clone(),
                (),
                Self::users_to_inner,
                Self::users_to_outer,
            ))
        }
        fn users_to_inner(
            outer: Result<Response, WrongResponseType>,
        ) -> Result<<UsersService as Rpc>::Response, WrongResponseType> {
            match outer {
                Ok(Response::Users(inner)) => Ok(inner),
                Ok(other) => Err(WrongResponseType::new("users", other.fn_name()).into()),
                Err(err) => Err(err.in_subservice("users")),
            }
        }
        fn users_to_outer((): (), inner: <UsersService as Rpc>::Request) -> Request {
            Request::Users(inner)
        }
        pub async fn login(
            &self,
            username: String,
            password: String,
        ) -> Result<Option<LoginToken>, _Client::Error> {
            match self.0.send(Request::Login(username, password)).await? {
                Response::Login(value) => Ok(value),
                other => Err(WrongResponseType::new("login", other.fn_name()).into()),
            }
        }
    }
    /// This is the blocking client for the service, it produces requests from method calls
    /// (including chained method calls) and sends the requests with the given
    /// [transport](::trait_link::AsyncClient) before returning the response
    ///
    /// The return value is always wrapped in a result: `Result<T, _Client::Error>` where `T` is the service return value
    #[derive(Debug, Copy, Clone)]
    pub struct ApiServiceBlockingClient<_Client>(_Client);
    impl<_Client: BlockingClient<Request, Response>> ApiServiceBlockingClient<_Client> {
        pub fn users(
            &self,
        ) -> <UsersService as Rpc>::BlockingClient<
            MappedClient<
                _Client,
                <UsersService as Rpc>::Request,
                Request,
                <UsersService as Rpc>::Response,
                Response,
                (),
            >,
        > {
            UsersService::blocking_client(MappedClient::new(
                self.0.clone(),
                (),
                Self::users_to_inner,
                Self::users_to_outer,
            ))
        }
        fn users_to_inner(
            outer: Result<Response, WrongResponseType>,
        ) -> Result<<UsersService as Rpc>::Response, WrongResponseType> {
            match outer {
                Ok(Response::Users(inner)) => Ok(inner),
                Ok(other) => Err(WrongResponseType::new("users", other.fn_name()).into()),
                Err(err) => Err(err.in_subservice("users")),
            }
        }
        fn users_to_outer((): (), inner: <UsersService as Rpc>::Request) -> Request {
            Request::Users(inner)
        }
        pub fn login(
            &self,
            username: String,
            password: String,
        ) -> Result<Option<LoginToken>, _Client::Error> {
            match self.0.send(Request::Login(username, password))? {
                Response::Login(value) => Ok(value),
                other => Err(WrongResponseType::new("login", other.fn_name()).into()),
            }
        }
    }
}
#[allow(unused_imports, reason = "These might not always be used, but they should be available in this module anyway")]
pub use users_service::{
    UsersService, UsersServiceAsyncClient, UsersServiceBlockingClient, UsersServiceServer,
};
#[allow(unused_imports, reason = "These might not always be used, but it's easier to include always")]
mod users_service {
    use super::*;
    use ::trait_link::{
        Rpc,
        client::{AsyncClient, BlockingClient, MappedClient, WrongResponseType},
        serde::{Deserialize, Serialize},
        server::Handler,
    };
    use std::marker::PhantomData;
    /// This is the [Rpc](::trait_link::Rpc) definition for this service
    pub struct UsersService;
    impl Rpc for UsersService {
        type AsyncClient<_Client: AsyncClient<Self::Request, Self::Response>> =
            UsersServiceAsyncClient<_Client>;
        type BlockingClient<_Client: BlockingClient<Self::Request, Self::Response>> =
            UsersServiceBlockingClient<_Client>;
        type Request = Request;
        type Response = Response;
        fn async_client<_Client: AsyncClient<Request, Response>>(
            transport: _Client,
        ) -> UsersServiceAsyncClient<_Client> {
            UsersServiceAsyncClient(transport)
        }
        fn blocking_client<_Client: BlockingClient<Request, Response>>(
            transport: _Client,
        ) -> UsersServiceBlockingClient<_Client> {
            UsersServiceBlockingClient(transport)
        }
    }
    impl UsersService {
        /// Create a new [Handler](trait_link::Handler) for the service
        pub fn server(server: impl UsersServiceServer) -> impl Handler<Rpc = Self> {
            UsersServiceHandler(server)
        }
    }
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(crate = "::trait_link::serde")]
    #[serde(tag = "method", content = "args")]
    pub enum Request {
        #[serde(rename = "new")]
        New(NewUser),
        #[serde(rename = "list")]
        List(),
        #[serde(rename = "by_id")]
        ById(u64, <UserService as Rpc>::Request),
        #[serde(rename = "current")]
        Current(LoginToken, <UserService as Rpc>::Request),
    }
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(crate = "::trait_link::serde")]
    #[serde(tag = "method", content = "result")]
    pub enum Response {
        #[serde(rename = "new")]
        New(User),
        #[serde(rename = "list")]
        List(Vec<User>),
        #[serde(rename = "by_id")]
        ById(<UserService as Rpc>::Response),
        #[serde(rename = "current")]
        Current(<UserService as Rpc>::Response),
    }
    impl Response {
        fn fn_name(&self) -> &'static str {
            match self {
                Self::New(..) => "new",
                Self::List(..) => "list",
                Self::ById(..) => "by_id",
                Self::Current(..) => "current",
            }
        }
    }
    /// This is the trait which is used by the server side in order to serve the client
    pub trait UsersServiceServer: Send + Sync {
        fn new(&self, user: NewUser) -> impl Future<Output = User> + Send;
        fn list(&self) -> impl Future<Output = Vec<User>> + Send;
        fn by_id(&self, id: u64)
        -> impl Future<Output = impl Handler<Rpc = UserService>> + Send;
        fn current(
            &self,
            token: LoginToken,
        ) -> impl Future<Output = impl Handler<Rpc = UserService>> + Send;
    }
    /// A [Handler](Handler) which handles requests/responses for a given service
    #[derive(Debug, Clone)]
    pub struct UsersServiceHandler<_Server>(_Server);
    impl<_Server: UsersServiceServer> Handler for UsersServiceHandler<_Server> {
        type Rpc = UsersService;
        async fn handle(&self, request: Request) -> Response {
            match request {
                Request::New(user) => Response::New(self.0.new(user).await),
                Request::List() => Response::List(self.0.list().await),
                Request::ById(id, request) => {
                    let response = self.0.by_id(id).await.handle(request).await;
                    Response::ById(response)
                }
                Request::Current(token, request) => {
                    let response = self.0.current(token).await.handle(request).await;
                    Response::Current(response)
                }
            }
        }
    }



    /// This is the async client for the service, it produces requests from method calls
    /// (including chained method calls) and sends the requests with the given
    /// [transport](::trait_link::AsyncClient) before returning the response
    ///
    /// The return value is always wrapped in a result: `Result<T, _Client::Error>` where `T` is the service return value
    #[derive(Debug, Copy, Clone)]
    pub struct UsersServiceAsyncClient<_Client>(_Client);
    impl<_Client: AsyncClient<Request, Response>> UsersServiceAsyncClient<_Client> {
        pub async fn new(&self, user: NewUser) -> Result<User, _Client::Error> {
            match self.0.send(Request::New(user)).await? {
                Response::New(value) => Ok(value),
                other => Err(WrongResponseType::new("new", other.fn_name()).into()),
            }
        }
        pub async fn list(&self) -> Result<Vec<User>, _Client::Error> {
            match self.0.send(Request::List()).await? {
                Response::List(value) => Ok(value),
                other => Err(WrongResponseType::new("list", other.fn_name()).into()),
            }
        }
        pub fn by_id(
            &self,
            id: u64,
        ) -> <UserService as Rpc>::AsyncClient<
            MappedClient<
                _Client,
                <UserService as Rpc>::Request,
                Request,
                <UserService as Rpc>::Response,
                Response,
                (u64,),
            >,
        > {
            UserService::async_client(MappedClient::new(
                self.0.clone(),
                (id,),
                Self::by_id_to_inner,
                Self::by_id_to_outer,
            ))
        }
        fn by_id_to_inner(
            outer: Result<Response, WrongResponseType>,
        ) -> Result<<UserService as Rpc>::Response, WrongResponseType> {
            match outer {
                Ok(Response::ById(inner)) => Ok(inner),
                Ok(other) => Err(WrongResponseType::new("by_id", other.fn_name()).into()),
                Err(err) => Err(err.in_subservice("by_id")),
            }
        }
        fn by_id_to_outer((id,): (u64,), inner: <UserService as Rpc>::Request) -> Request {
            Request::ById(id, inner)
        }
        pub fn current(
            &self,
            token: LoginToken,
        ) -> <UserService as Rpc>::AsyncClient<
            MappedClient<
                _Client,
                <UserService as Rpc>::Request,
                Request,
                <UserService as Rpc>::Response,
                Response,
                (LoginToken,),
            >,
        > {
            UserService::async_client(MappedClient::new(
                self.0.clone(),
                (token,),
                Self::current_to_inner,
                Self::current_to_outer,
            ))
        }
        fn current_to_inner(
            outer: Result<Response, WrongResponseType>,
        ) -> Result<<UserService as Rpc>::Response, WrongResponseType> {
            match outer {
                Ok(Response::Current(inner)) => Ok(inner),
                Ok(other) => Err(WrongResponseType::new("current", other.fn_name()).into()),
                Err(err) => Err(err.in_subservice("current")),
            }
        }
        fn current_to_outer(
            (token,): (LoginToken,),
            inner: <UserService as Rpc>::Request,
        ) -> Request {
            Request::Current(token, inner)
        }
    }
    /// This is the blocking client for the service, it produces requests from method calls
    /// (including chained method calls) and sends the requests with the given
    /// [transport](::trait_link::AsyncClient) before returning the response
    ///
    /// The return value is always wrapped in a result: `Result<T, _Client::Error>` where `T` is the service return value
    #[derive(Debug, Copy, Clone)]
    pub struct UsersServiceBlockingClient<_Client>(_Client);
    impl<_Client: BlockingClient<Request, Response>> UsersServiceBlockingClient<_Client> {
        pub fn new(&self, user: NewUser) -> Result<User, _Client::Error> {
            match self.0.send(Request::New(user))? {
                Response::New(value) => Ok(value),
                other => Err(WrongResponseType::new("new", other.fn_name()).into()),
            }
        }
        pub fn list(&self) -> Result<Vec<User>, _Client::Error> {
            match self.0.send(Request::List())? {
                Response::List(value) => Ok(value),
                other => Err(WrongResponseType::new("list", other.fn_name()).into()),
            }
        }
        pub fn by_id(
            &self,
            id: u64,
        ) -> <UserService as Rpc>::BlockingClient<
            MappedClient<
                _Client,
                <UserService as Rpc>::Request,
                Request,
                <UserService as Rpc>::Response,
                Response,
                (u64,),
            >,
        > {
            UserService::blocking_client(MappedClient::new(
                self.0.clone(),
                (id,),
                Self::by_id_to_inner,
                Self::by_id_to_outer,
            ))
        }
        fn by_id_to_inner(
            outer: Result<Response, WrongResponseType>,
        ) -> Result<<UserService as Rpc>::Response, WrongResponseType> {
            match outer {
                Ok(Response::ById(inner)) => Ok(inner),
                Ok(other) => Err(WrongResponseType::new("by_id", other.fn_name()).into()),
                Err(err) => Err(err.in_subservice("by_id")),
            }
        }
        fn by_id_to_outer((id,): (u64,), inner: <UserService as Rpc>::Request) -> Request {
            Request::ById(id, inner)
        }
        pub fn current(
            &self,
            token: LoginToken,
        ) -> <UserService as Rpc>::BlockingClient<
            MappedClient<
                _Client,
                <UserService as Rpc>::Request,
                Request,
                <UserService as Rpc>::Response,
                Response,
                (LoginToken,),
            >,
        > {
            UserService::blocking_client(MappedClient::new(
                self.0.clone(),
                (token,),
                Self::current_to_inner,
                Self::current_to_outer,
            ))
        }
        fn current_to_inner(
            outer: Result<Response, WrongResponseType>,
        ) -> Result<<UserService as Rpc>::Response, WrongResponseType> {
            match outer {
                Ok(Response::Current(inner)) => Ok(inner),
                Ok(other) => Err(WrongResponseType::new("current", other.fn_name()).into()),
                Err(err) => Err(err.in_subservice("current")),
            }
        }
        fn current_to_outer(
            (token,): (LoginToken,),
            inner: <UserService as Rpc>::Request,
        ) -> Request {
            Request::Current(token, inner)
        }
    }
}
#[allow(unused_imports, reason = "These might not always be used, but they should be available in this module anyway")]
pub use user_service::{
    UserService, UserServiceAsyncClient, UserServiceBlockingClient, UserServiceServer,
};
#[allow(unused_imports, reason = "These might not always be used, but it's easier to include always")]
mod user_service {
    use super::*;
    use ::trait_link::{
        Rpc,
        client::{AsyncClient, BlockingClient, MappedClient, WrongResponseType},
        serde::{Deserialize, Serialize},
        server::Handler,
    };
    use std::marker::PhantomData;
    /// This is the [Rpc](::trait_link::Rpc) definition for this service
    pub struct UserService;
    impl Rpc for UserService {
        type AsyncClient<_Client: AsyncClient<Self::Request, Self::Response>> =
            UserServiceAsyncClient<_Client>;
        type BlockingClient<_Client: BlockingClient<Self::Request, Self::Response>> =
            UserServiceBlockingClient<_Client>;
        type Request = Request;
        type Response = Response;
        fn async_client<_Client: AsyncClient<Request, Response>>(
            transport: _Client,
        ) -> UserServiceAsyncClient<_Client> {
            UserServiceAsyncClient(transport)
        }
        fn blocking_client<_Client: BlockingClient<Request, Response>>(
            transport: _Client,
        ) -> UserServiceBlockingClient<_Client> {
            UserServiceBlockingClient(transport)
        }
    }
    impl UserService {
        /// Create a new [Handler](trait_link::Handler) for the service
        pub fn server(server: impl UserServiceServer) -> impl Handler<Rpc = Self> {
            UserServiceHandler(server)
        }
    }
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(crate = "::trait_link::serde")]
    #[serde(tag = "method", content = "args")]
    pub enum Request {
        #[serde(rename = "get")]
        Get(),
        #[serde(rename = "update")]
        Update(UserUpdate),
        #[serde(rename = "delete")]
        Delete(),
    }
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(crate = "::trait_link::serde")]
    #[serde(tag = "method", content = "result")]
    pub enum Response {
        #[serde(rename = "get")]
        Get(Result<User, UserNotFound>),
        #[serde(rename = "update")]
        Update(Result<User, UserNotFound>),
        #[serde(rename = "delete")]
        Delete(Result<User, UserNotFound>),
    }
    impl Response {
        fn fn_name(&self) -> &'static str {
            match self {
                Self::Get(..) => "get",
                Self::Update(..) => "update",
                Self::Delete(..) => "delete",
            }
        }
    }
    /// This is the trait which is used by the server side in order to serve the client
    pub trait UserServiceServer: Send + Sync {
        fn get(&self) -> impl Future<Output = Result<User, UserNotFound>> + Send;
        fn update(
            &self,
            user: UserUpdate,
        ) -> impl Future<Output = Result<User, UserNotFound>> + Send;
        fn delete(&self) -> impl Future<Output = Result<User, UserNotFound>> + Send;
    }
    /// A [Handler](Handler) which handles requests/responses for a given service
    #[derive(Debug, Clone)]
    pub struct UserServiceHandler<_Server>(_Server);
    impl<_Server: UserServiceServer> Handler for UserServiceHandler<_Server> {
        type Rpc = UserService;
        async fn handle(&self, request: Request) -> Response {
            match request {
                Request::Get() => Response::Get(self.0.get().await),
                Request::Update(user) => Response::Update(self.0.update(user).await),
                Request::Delete() => Response::Delete(self.0.delete().await),
            }
        }
    }



    /// This is the async client for the service, it produces requests from method calls
    /// (including chained method calls) and sends the requests with the given
    /// [transport](::trait_link::AsyncClient) before returning the response
    ///
    /// The return value is always wrapped in a result: `Result<T, _Client::Error>` where `T` is the service return value
    #[derive(Debug, Copy, Clone)]
    pub struct UserServiceAsyncClient<_Client>(_Client);
    impl<_Client: AsyncClient<Request, Response>> UserServiceAsyncClient<_Client> {
        pub async fn get(&self) -> Result<Result<User, UserNotFound>, _Client::Error> {
            match self.0.send(Request::Get()).await? {
                Response::Get(value) => Ok(value),
                other => Err(WrongResponseType::new("get", other.fn_name()).into()),
            }
        }
        pub async fn update(
            &self,
            user: UserUpdate,
        ) -> Result<Result<User, UserNotFound>, _Client::Error> {
            match self.0.send(Request::Update(user)).await? {
                Response::Update(value) => Ok(value),
                other => Err(WrongResponseType::new("update", other.fn_name()).into()),
            }
        }
        pub async fn delete(&self) -> Result<Result<User, UserNotFound>, _Client::Error> {
            match self.0.send(Request::Delete()).await? {
                Response::Delete(value) => Ok(value),
                other => Err(WrongResponseType::new("delete", other.fn_name()).into()),
            }
        }
    }
    /// This is the blocking client for the service, it produces requests from method calls
    /// (including chained method calls) and sends the requests with the given
    /// [transport](::trait_link::AsyncClient) before returning the response
    ///
    /// The return value is always wrapped in a result: `Result<T, _Client::Error>` where `T` is the service return value
    #[derive(Debug, Copy, Clone)]
    pub struct UserServiceBlockingClient<_Client>(_Client);
    impl<_Client: BlockingClient<Request, Response>> UserServiceBlockingClient<_Client> {
        pub fn get(&self) -> Result<Result<User, UserNotFound>, _Client::Error> {
            match self.0.send(Request::Get())? {
                Response::Get(value) => Ok(value),
                other => Err(WrongResponseType::new("get", other.fn_name()).into()),
            }
        }
        pub fn update(
            &self,
            user: UserUpdate,
        ) -> Result<Result<User, UserNotFound>, _Client::Error> {
            match self.0.send(Request::Update(user))? {
                Response::Update(value) => Ok(value),
                other => Err(WrongResponseType::new("update", other.fn_name()).into()),
            }
        }
        pub fn delete(&self) -> Result<Result<User, UserNotFound>, _Client::Error> {
            match self.0.send(Request::Delete())? {
                Response::Delete(value) => Ok(value),
                other => Err(WrongResponseType::new("delete", other.fn_name()).into()),
            }
        }
    }
}
