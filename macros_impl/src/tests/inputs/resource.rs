#[rpc]
pub trait Resources<T> {
    fn subscribe() -> impl Stream<T>;
    fn list() -> Vec<T>;
    fn get(id: u64) -> Option<T>;
    fn new(value: T);
}