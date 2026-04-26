use trait_rpc::client::browser::Browser;
use trait_rpc::{Rpc, client};
use trait_rpc::format::json::Json;

include!("traits/todo.rs");

fn main() {
    wasm_bindgen_futures::spawn_local(async move {
        let client = TodoService::async_client(
            client::builder()
                .non_blocking()
                .transport(
                    Browser::builder()
                        .url("http://localhost:8080/api/todo")
                        .method("POST")
                        .build()
                )
                .format(&Json)
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
    });
}