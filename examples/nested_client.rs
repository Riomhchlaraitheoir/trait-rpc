use trait_link::client::reqwest::Reqwest;
use trait_link::format::Json;
use trait_link::Rpc;

include!("traits/nested.rs");

#[tokio::main]
async fn main() {
    let client = ApiService::async_client(
        trait_link::client::builder()
            .non_blocking()
            .transport(
                Reqwest::builder()
                    .url("http://localhost:8080/api")
                    .build()
            )
            .format(Json)
            .build()
    );
    let dylan = client
        .users()
        .new(NewUser {
            name: "Dylan".to_string(),
            username: "dylan".to_string(),
            password: "secret".to_string(),
        })
        .await
        .expect("failed to create a new user");
    assert_eq!(dylan.name, "Dylan");
    assert_eq!(dylan.username, "dylan");
    println!("Successfully created a user");
    let users = client.users().list().await.expect("Error getting users");
    assert_eq!(users.len(), 1);
    let user = users[0].clone();
    assert_eq!(user, dylan);
    println!("user is included in list");

    let token = client
        .login("dylan".to_string(), "secret".to_string())
        .await
        .expect("login request failed")
        .expect("incorrect login details");
    println!("Successfully logged in as user");
    let current_user = client
        .users()
        .current(token)
        .get()
        .await
        .expect("failed to get user")
        .expect("user not found");
    println!("Current user: {}", current_user.name);
    assert_eq!(current_user, dylan);

    let dylan_service = client.users().by_id(dylan.id);
    let fetched = dylan_service
        .get()
        .await
        .expect("Error getting user")
        .expect("user not found");
    assert_eq!(fetched, dylan);
    println!("Successfully fetched user");
    let deleted = dylan_service
        .delete()
        .await
        .expect("Error deleting user")
        .expect("user not found");
    assert_eq!(deleted, dylan);
    println!("Successfully deleted user");
    dylan_service
        .get()
        .await
        .expect("Error getting user")
        .expect_err("User not deleted");
    println!("Confirmed user deleted")
}