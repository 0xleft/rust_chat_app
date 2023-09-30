use tokio::io::{AsyncWriteExt, Result};
use tokio_tungstenite::{connect_async, tungstenite::{protocol::Message, http::request}};
use futures_util::{StreamExt, SinkExt};

#[tokio::main]
pub async fn main() -> Result<()> {

    let server = ask_for_input("Enter server address:".to_string());
    let server = server.trim();

    // test connection
    let test_url = url::Url::parse(format!("http://{}/test", server).as_str()).unwrap();
    let test_response = reqwest::get(test_url).await.unwrap();

    if test_response.text().await.unwrap() != "healthy :)" {
        println!("Server not found.");
        return Ok(());
    }
    println!("Server found.");

    // register logic
    let register_url = url::Url::parse(format!("http://{}/register", server).as_str()).unwrap();
    let register: bool = ask_for_input("Do you want to register? (y/n)".to_string()).trim() == "y";

    let username = ask_for_input("Enter your username:".to_string());
    let password = ask_for_input("Enter your password:".to_string());

    println!("Registering...");
    if register {
        let register_response = reqwest::Client::new()
            .post(register_url)
            .json(&serde_json::json!({
                "username": username.trim(),
                "password": password.trim()
            }))
            .send()
            .await
            .unwrap();

        if register_response.status().is_success() {
            println!("Registered successfully!");
        } else {
            println!("Registration failed.");
            return Ok(());
        }
    }

    // chat logic
    println!("Connecting to chat...");
    let ws_url = url::Url::parse(format!("ws://{}/chat", server).as_str()).unwrap();
    let (ws_stream, _response) = connect_async(ws_url).await.expect("Failed to connect");
    let (mut tx, rx) = ws_stream.split();
    println!("Connected to chat.\r");

    // json login
    println!("Logging in...");
    tx.send(Message::Text(serde_json::json!({
        "username": username.trim(),
        "password": password.trim()
    }).to_string())).await.unwrap();
    println!("Logged in.\r");

    // read loop
    let read_future = rx.for_each(|message| async {
        // hadle message
    });

    // write loop
    

    read_future.await;
    Ok(())


}

fn ask_for_input(prompt: String) -> String {
    println!("{}", prompt);
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);
    input
}