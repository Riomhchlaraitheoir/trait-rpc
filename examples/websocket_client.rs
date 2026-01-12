#![doc = include_str!("./examples.md")]

use trait_rpc::{Rpc, client};
use trait_rpc::client::websocket::Websocket;
use trait_rpc::format::json::Json;

include!("traits/todo.rs");

#[tokio::main]
async fn main() {
    let client = TodoService::async_client(
        client::builder()
            .non_blocking()
            .transport(
                Websocket::new("ws://127.0.0.1:8080".parse().unwrap(), Json).await.expect("failed to start connection")
            )
            .format(Json)
            .build()
    );
    for todo in client.get_todos().await.expect("get_todos failed") {
        println!("{todo:?}");
    }
    if let Some(todo) = client.get_todo("next".to_string()).await.expect("get_todo failed") {
        println!("{todo:?}");
    }
    client.new_todo(Todo {
        name: "Some task".to_string(),
        description: "A description of the task".to_string(),
    }).await.expect("new_todo failed");
}