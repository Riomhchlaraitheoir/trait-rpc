#[rpc]
pub trait Resources<T> {
    fn list(&self) -> Vec<T>;
    fn get(&self, id: u64) -> Option<T>;
    fn new(&self, value: T);
}