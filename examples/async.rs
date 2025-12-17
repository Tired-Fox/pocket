use pocket::{Client, non_blocking::ExtendAuth};
use serde_json::Value;

#[tokio::main]
async fn main() {
    let pocket = Client::new("http://localhost:3000");

    let pocket = pocket
        .collection("users")
        .auth_with_password("-", "-")
        .await
        .unwrap();

    if pocket.is_expired() {
        pocket.refresh().await.unwrap();
    }

    pocket
        .collection("item")
        .get_list::<Value>(Default::default())
        .await
        .unwrap();

    pocket.authenticate(token)
}