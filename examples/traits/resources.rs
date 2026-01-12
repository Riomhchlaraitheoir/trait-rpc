use macros::rpc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Eq, PartialEq)]
pub struct Book {
    pub id: u64,
    pub title: String,
    pub author: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Eq, PartialEq)]
pub struct Author {
    pub id: u64,
    pub name: String,
    pub quote: String,
}

#[rpc]
pub trait Resources<T> {
    fn subscribe(&self) -> Stream<T>;
    fn list(&self) -> Vec<T>;
    fn get(&self, id: u64) -> Option<T>;
    fn new(&self, value: T);
}

// include expanded form here for debugging:
// include!("../../macros_impl/src/tests/outputs/resource.rs");
