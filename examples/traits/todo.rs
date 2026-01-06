use macros::rpc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "::trait_link::serde")]
#[allow(unused)]
struct Todo {
    name: String,
    description: String,
}

#[rpc]
/// A service for managing to-do items
trait TodoService {
    /// Get a list of to-do items
    fn get_todos(&self) -> Vec<Todo>;
    /// Get a to-do item by name, returns None if no to-do item with the given name exists
    fn get_todo(&self, name: String) -> Option<Todo>;
    /// Create a new to-do item
    fn new_todo(&self, todo: Todo);
}

// include!("../../macros_impl/src/tests/outputs/simple.rs");