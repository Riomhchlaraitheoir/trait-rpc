
trait TodoServer {
    async fn get_todos(&self) -> Vec<Todo>;
    async fn get_todo(&self, name: String) -> Option<Todo>;
    async fn new_todo(&self, todo: Todo) -> ();
}