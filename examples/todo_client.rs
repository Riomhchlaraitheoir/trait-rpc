use trait_link::reqwest::Client;
use trait_link::*;
use trait_link::serde::{Serialize, Deserialize};
use crate::todo_lib::Todo;

mod todo_lib;
use todo_lib::*;

#[tokio::main]
async fn main() {
    let client = TodoServerClient::new(Client::new("http://localhost:8080/api/todo"));
    for todo in client.get_todos().await.expect("get_todos failed") {
        println!("{todo:?}")
    }
    if let Some(todo) = client.get_todo("next".to_string()).await.expect("get_todo failed") {
        println!("{todo:?}")
    }
    client.new_todo(Todo {
        title: "".to_string(),
        description: "".to_string(),
        done: false,
        deadline: Default::default(),
    }).await.expect("new_todo failed");
}
