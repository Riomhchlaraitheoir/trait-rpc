#![allow(refining_impl_trait_internal)]

use axum::Router;
use axum::http::Method;
use futures::future::{Either, ready};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ops::Deref;
use tokio::sync::RwLock;
use trait_rpc::{RpcWithServer};
use trait_rpc::format::{cbor::Cbor, json::Json};
use trait_rpc::server::axum::Axum;

include!("traits/nested.rs");

#[tokio::main]
async fn main() {
    let app = Router::new().route_service(
        "/api",
        Axum::builder()
            .handler(ApiService::handler(Api::default()))
            .format(&Json)
            .format(&Cbor)
            .method(Method::POST)
            .build(),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

impl LoginToken {
    #[must_use]
    pub fn generate_new() -> Self {
        Self(String::from("<random UUID>"))
    }
}

impl Password {
    // no hashing used here since this is just intended to get the example compiling, not to be an actual example of password hashing
    #[must_use]
    pub fn new(password: &str) -> Self {
        Self {
            hash: password.to_string(),
            salt: String::new(),
        }
    }

    #[must_use]
    pub fn matches(&self, password: &str) -> bool {
        self.hash == password
    }
}

impl User {
    #[must_use]
    pub fn new(id: u64, user: NewUser) -> Self {
        Self {
            id,
            name: user.name,
            username: user.username,
            password: Password::new(&user.password),
        }
    }
    pub fn update(&mut self, update: UserUpdate) -> &'_ Self {
        if let Some(name) = update.name {
            self.name = name;
        }
        if let Some(username) = update.username {
            self.username = username;
        }
        if let Some(password) = update.password {
            self.password = Password::new(&password);
        }
        self
    }
}

#[derive(Default)]
struct Api {
    users: RwLock<HashMap<u64, User>>,
    tokens: RwLock<HashMap<LoginToken, u64>>,
}

impl ApiServiceServer for Api {
    async fn users(&self) -> impl UsersServiceServer {
        Users(self)
    }

    async fn login(&self, username: String, password: String) -> Option<LoginToken> {
        println!("received login request: username: {username}, password: {password}");
        let user = self
            .users
            .read()
            .await
            .values()
            .find(|&user| user.username == username)?
            .clone();
        if user.password.matches(&password) {
            let token = LoginToken::generate_new();
            self.tokens.write().await.insert(token.clone(), user.id);
            Some(token)
        } else {
            None
        }
    }
}

struct Users<'a>(&'a Api);

impl Deref for Users<'_> {
    type Target = Api;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl UsersServiceServer for Users<'_> {
    async fn new(&self, user: NewUser) -> User {
        println!("received user creation request: {user:?}");
        let mut users = self.users.write().await;
        let id = users.keys().copied().max().unwrap_or_default();
        let user = User::new(id, user);
        users.insert(id, user.clone());
        user
    }

    async fn list(&self) -> Vec<User> {
        println!("received user list request");
        self.users.read().await.values().cloned().collect()
    }

    async fn by_id(&self, user_id: u64) -> impl UserServiceServer {
        UserServer {
            api: self.0,
            user_id,
        }
    }

    async fn current(&self, token: LoginToken) -> impl UserServiceServer {
        self.tokens
            .read()
            .await
            .get(&token)
            .map(|&user_id| UserServer {
                api: self.0,
                user_id,
            })
            .ok_or(UserNotFound)
    }
}

struct UserServer<'a> {
    api: &'a Api,
    user_id: u64,
}

impl UserServiceServer for UserServer<'_> {
    async fn get(&self) -> Result<User, UserNotFound> {
        let Self { api, user_id } = *self;
        println!("received user get request, id: {user_id}");
        api.users
            .read()
            .await
            .get(&user_id)
            .cloned()
            .ok_or(UserNotFound)
    }

    async fn update(&self, user: UserUpdate) -> Result<User, UserNotFound> {
        let Self { api, user_id } = *self;
        println!("received user update request, id: {user_id}, update: {user:?}");
        if let Entry::Occupied(entry) = api.users.write().await.entry(user_id) {
            let updated = entry.into_mut().update(user);
            Ok(updated.clone())
        } else {
            Err(UserNotFound)
        }
    }

    async fn delete(&self) -> Result<User, UserNotFound> {
        let Self { api, user_id } = *self;
        println!("received user delete request, id: {user_id}");
        if let Entry::Occupied(entry) = api.users.write().await.entry(user_id) {
            let (_, user) = entry.remove_entry();
            Ok(user)
        } else {
            Err(UserNotFound)
        }
    }
}

impl<S, E> UserServiceServer for Result<S, E>
where
    Self: Send,
    S: UserServiceServer,
    E: Into<UserNotFound> + Clone + Sync,
{
    fn get(&self) -> impl Future<Output = Result<User, UserNotFound>> + Send {
        match self {
            Ok(ok) => Either::Left(ok.get()),
            Err(err) => Either::Right(ready(Err(err.clone().into()))),
        }
    }

    fn update(&self, user: UserUpdate) -> impl Future<Output = Result<User, UserNotFound>> + Send {
        match self {
            Ok(ok) => Either::Left(ok.update(user)),
            Err(err) => Either::Right(ready(Err(err.clone().into()))),
        }
    }

    fn delete(&self) -> impl Future<Output = Result<User, UserNotFound>> + Send {
        match self {
            Ok(ok) => Either::Left(ok.delete()),
            Err(err) => Either::Right(ready(Err(err.clone().into()))),
        }
    }
}
