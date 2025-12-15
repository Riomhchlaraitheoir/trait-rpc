use serde::{Deserialize, Serialize};
use trait_link::trait_link;

#[derive(Debug, Serialize, Deserialize)]
pub struct Todo {
    pub title: String,
    pub description: String,
    pub done: bool,
    pub deadline: chrono::NaiveDateTime,
}

#[trait_link]
pub trait TodoServer {
    async fn get_todos(&self) -> Vec<Todo>;
    async fn get_todo(&self, name: String) -> Option<Todo>;
    async fn new_todo(&self, todo: Todo);
}
