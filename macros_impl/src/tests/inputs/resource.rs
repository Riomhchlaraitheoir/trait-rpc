#[rpc]
pub trait Resources<T> {
    fn subscribe(&self) -> Stream<T>;
    fn list(&self) -> Vec<T>;
    fn get(&self, id: u64) -> Option<T>;
    fn new(&self, value: T);
}