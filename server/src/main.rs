use core::borrow;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use futures_util::lock::Mutex;
use futures_util::{SinkExt, StreamExt, TryFutureExt};
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::reply::Json;
use warp::ws::{Message, WebSocket};
use warp::Filter;
use serde::Serialize;
use serde::Deserialize;

#[derive(Serialize, Deserialize)]
pub struct User {
    username: String,
    password: String,
}

pub struct ConnectedUser {
    username: String,
    sender: mpsc::UnboundedSender<Message>,
}

#[derive(Serialize, Deserialize)]
pub struct ChatMessage {
    from: String,
    to: String,
    content: String,
}

type Users = Arc<RwLock<Vec<User>>>;
type ConnectedUsers = Arc<RwLock<Vec<ConnectedUser>>>;

#[tokio::main]
async fn main() {
    let users: Users = Users::default();
    let connected_users: ConnectedUsers = ConnectedUsers::default();

    // GET /chat -> websocket upgrade
    let chat = warp::path("chat")
        .and(warp::ws())
        .and(with_users(users.clone()))
        .and(with_connected_users(connected_users.clone()))
        .map(|ws: warp::ws::Ws, users, connected_users| {
            ws.on_upgrade(move |socket| user_connected(socket, users, connected_users))
        });

    // post /register
    let register_path = warp::path("register")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_users(users.clone()))
        .and_then(register_user);

    let test_path = warp::path("test")
        .and(warp::get())
        .map(|| {
            "healthy :)"
        });

    let routes = register_path.or(chat).or(test_path);

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

async fn register_user(user: User, users: Users) -> Result<impl warp::Reply, warp::Rejection> {
    // check if user exists
    let check_users = users.read().await;
    let found_user = check_users.iter().find(|&_user| _user.username == user.username);
    if found_user.is_some() {
        return Ok("User already exists".to_string());
    }
    drop(check_users);

    let mut users = users.write().await;
    let new_user = User {
        username: user.username.clone(),
        password: user.password.clone(),
    };
    users.push(new_user);
    print!("User registered: {}", user.username);
    Ok("User registered".to_string())
}

fn with_users(users: Users) -> impl Filter<Extract = (Users,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || users.clone())
}

fn with_connected_users(connected_users: ConnectedUsers) -> impl Filter<Extract = (ConnectedUsers,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || connected_users.clone())
}

async fn user_connected(ws: WebSocket, users: Users, connected_users: ConnectedUsers) {
    let (mut user_ws_tx, mut user_ws_rx) = ws.split();

    let (tx, rx) = mpsc::unbounded_channel();
    let mut rx = UnboundedReceiverStream::new(rx);

    tokio::task::spawn(async move {
        while let Some(message) = rx.next().await {
            user_ws_tx
                .send(message)
                .unwrap_or_else(|e| {
                    eprintln!("websocket send error: {}", e);
                })
                .await;
        }
    });

    // handle login
    let login_message = user_ws_rx.next().await.unwrap().unwrap();
    let login_message = String::from_utf8(login_message.as_bytes().to_vec()).unwrap();

    // dumb way to prevent bad json
    let login_message: User = serde_json::from_str(&login_message).unwrap_or(User {
        username: "".to_string(),
        password: "".to_string(),
    });

    let _users = users.write().await;
    println!("User login: {}:{}", login_message.username, login_message.password);

    let user = _users.iter().find(|&user| user.username == login_message.username && user.password == login_message.password).unwrap().clone();
    println!("User logged in {}", user.username);

    let user = User {
        username: user.username.clone(),
        password: user.password.clone(),
    };

    // insert user into connected users
    let mut _connected_users = connected_users.write().await;
    _connected_users.push(ConnectedUser {
        username: user.username.clone(),
        sender: tx.clone(),
    });
    drop(_connected_users);
    drop(_users);

    while let Some(result) = user_ws_rx.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("websocket error(uid=): {}", e);
                break;
            }
        };
        let msg = String::from_utf8(msg.as_bytes().to_vec()).unwrap();
        let msg: ChatMessage = serde_json::from_str(&msg).unwrap_or(ChatMessage {
            from: "".to_string(),
            to: "".to_string(),
            content: "".to_string(),
        });
        println!("Message from {} to {}: {}", msg.from, msg.to, msg.content);
        send_message(&user, msg, connected_users.clone()).await;
    }
    
    // remove user from connected users
    println!("User disconnected {}", user.username);
    let mut _connected_users = connected_users.write().await;
    let index = _connected_users.iter().position(|user| user.username == user.username).unwrap();
    _connected_users.remove(index);
    drop(_connected_users);
}

async fn send_message(user: &User, mut message: ChatMessage, connected_users: ConnectedUsers) {
    let connected_users = connected_users.read().await;
    let connected_user = connected_users.iter().find(|&user| user.username == message.to);
    if connected_user.is_none() {
        println!("User not found");
        return;
    }
    let connected_user = connected_user.unwrap();
    // prevent spoofing 
    message.from = user.username.clone();
    // send a json message to the connected user
    connected_user.sender.send(Message::text(serde_json::to_string(&message).unwrap())).unwrap();
}