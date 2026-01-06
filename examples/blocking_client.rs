use axum::http::Method;
use trait_link::client::reqwest_blocking::ReqwestBlocking;
use trait_link::*;
use trait_link::format::Json;

include!("traits/todo.rs");

fn main() {
    let client = TodoService::blocking_client(
        client::builder()
            .blocking()
            .transport(
                ReqwestBlocking::builder()
                    .url("http://localhost:8080/api/todo")
                    .method(Method::POST)
                    .build()
            )
            .format(Json)
            .build()
    );
    for todo in client.get_todos().expect("get_todos failed") {
        println!("{todo:?}")
    }
    if let Some(todo) = client.get_todo("next".to_string()).expect("get_todo failed") {
        println!("{todo:?}")
    }
    client.new_todo(Todo {
        name: "Some task".to_string(),
        description: "A description of the task".to_string(),
    }).expect("new_todo failed");
}