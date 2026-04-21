use trait_rpc::rpc;

#[derive(Debug, Clone, Eq, PartialEq, Hash, serde::Deserialize, serde::Serialize)]
pub struct LoginToken(String);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Eq, PartialEq)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub username: String,
    #[serde(skip)]
    pub password: Password,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct NewUser {
    pub name: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct UserUpdate {
    pub name: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct Password {
    hash: String,
    salt: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct UserNotFound;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct LoginExpired;








#[rpc]
pub trait ApiService {
    fn users() -> impl UsersService;
    fn login(username: String, password: String) -> Option<LoginToken>;
}

#[rpc]
pub trait UsersService {
    fn new(user: NewUser) -> User;
    fn list() -> Vec<User>;
    fn by_id(id: u64) -> impl UserService;
    fn current(token: LoginToken) -> impl UserService;
}

#[rpc]
pub trait UserService {
    fn get() -> Result<User, UserNotFound>;
    fn update(user: UserUpdate) -> Result<User, UserNotFound>;
    fn delete() -> Result<User, UserNotFound>;
}

// include expanded form here for debugging:
// include!("../../macros_impl/src/tests/outputs/nested.rs");