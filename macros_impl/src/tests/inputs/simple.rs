#[rpc]
/// A service for managing to-do items
pub trait TodoService {
    /// Get a list of to-do items
    fn get_todos() -> Vec<Todo>;
    /// Get a to-do item by name, returns None if no to-do item with the given name exists
    fn get_todo(name: String) -> Option<Todo>;
    /// Create a new to-do item
    fn new_todo(todo: Todo);
}