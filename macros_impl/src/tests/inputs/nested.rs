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