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