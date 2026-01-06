use trait_link::client::reqwest::Reqwest;
use trait_link::*;
use trait_link::format::Json;

include!("traits/todo.rs");

#[tokio::main]
async fn main() {
    let client = TodoService::async_client(
        client::builder()
            .non_blocking()
            .transport(
                Reqwest::builder()
                    .url("http://localhost:8080/api/todo")
                    .build()
            )
            .format(Json)
            .build()
    );
    for todo in client.get_todos().await.expect("get_todos failed") {
        println!("{todo:?}")
    }
    if let Some(todo) = client.get_todo("next".to_string()).await.expect("get_todo failed") {
        println!("{todo:?}")
    }
    client.new_todo(Todo {
        name: "Some task".to_string(),
        description: "A description of the task".to_string(),
    }).await.expect("new_todo failed");
}