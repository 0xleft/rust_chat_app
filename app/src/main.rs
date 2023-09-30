use std::os::windows::prelude::AsHandle;

use tokio::io::{AsyncWriteExt, Result};
use tokio_tungstenite::{connect_async, tungstenite::{protocol::Message, http::request}};
use futures_util::{StreamExt, SinkExt};
use futures_util::{future, pin_mut};
use std::io::{self, Write};

#[tokio::main]
async fn main() {

    let server = ask_for_input("Enter server address: ".to_string());
    let server = server.trim();

    // test connection
    let test_url = url::Url::parse(format!("http://{}/test", server).as_str()).unwrap();
    let test_response = reqwest::get(test_url).await.unwrap();

    if test_response.text().await.unwrap() != "healthy :)" {
        println!("Server not found.");
        return;
    }
    println!("Server found.");

    // register logic
    let register_url = url::Url::parse(format!("http://{}/register", server).as_str()).unwrap();
    let register: bool = ask_for_input("Do you want to register? (y/n) ".to_string()).trim() == "y";

    let username = ask_for_input("Enter your username: ".to_string());
    let password = ask_for_input("Enter your password: ".to_string());

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
            return;
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

    let (stdin_tx, stdin_rx) = futures_channel::mpsc::unbounded();
    tokio::spawn(read_stdin(stdin_tx));

    let stdin_to_ws = stdin_rx.map(Ok).forward(tx);
    let ws_to_stdout = {
        rx.for_each(|message| async {
            let data = message.unwrap().into_text().unwrap();
            // parse json
            let json: serde_json::Value = serde_json::from_str(&data).unwrap();
            let from = json["from"].as_str().unwrap();
            let content = json["content"].as_str().unwrap();
            println!("{}: {}", from, content);
        })
    };

    // from example
    pin_mut!(stdin_to_ws, ws_to_stdout);
    future::select(stdin_to_ws, ws_to_stdout).await;
}

async fn read_stdin(mut tx: futures_channel::mpsc::UnboundedSender<Message>) {
    loop {
        let input = ask_for_input("$ ".to_string());

        // commands
        if input == "/exit" {
            println!("Exiting...");
            break;
        }

        if input.starts_with("/send") {
            // /send test2 hello there -> {"from":"test","to":"test2","content":"hello there"}
            let mut input = input.split_whitespace();
            input.next(); // skip /send
            let to = input.next().unwrap();
            let content = input.collect::<Vec<&str>>().join(" ");
            let json = serde_json::json!({
                "to": to,
                "content": content,
                "from": "",
            }).to_string();
            tx.send(Message::Text(json)).await.unwrap();
            continue;
        }
    }
}

fn ask_for_input(prompt: String) -> String {
    let mut input = String::new();
    print!("{}", prompt);
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}