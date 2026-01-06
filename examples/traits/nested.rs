use trait_link::rpc;

#[derive(Debug, Clone, Eq, PartialEq, Hash, serde::Deserialize, serde::Serialize)]
pub struct LoginToken(String);

impl LoginToken {
    pub fn generate_new() -> Self {
        Self(String::from("<random UUID>"))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Eq, PartialEq)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub username: String,
    #[serde(skip)]
    pub password: Password,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct NewUser {
    pub name: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
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

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct UserNotFound;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct LoginExpired;








#[rpc]
pub trait ApiService {
    fn users(&self) -> impl UsersService;
    fn login(&self, username: String, password: String) -> Option<LoginToken>;
}

#[rpc]
pub trait UsersService {
    fn new(&self, user: NewUser) -> User;
    fn list(&self) -> Vec<User>;
    fn by_id(&self, id: u64) -> impl UserService;
    fn current(&self, token: LoginToken) -> impl UserService;
}

#[rpc]
pub trait UserService {
    fn get(&self) -> Result<User, UserNotFound>;
    fn update(&self, user: UserUpdate) -> Result<User, UserNotFound>;
    fn delete(&self) -> Result<User, UserNotFound>;
}

// include expanded form here for debugging:
// include!("../../macros_impl/src/tests/outputs/nested.rs");