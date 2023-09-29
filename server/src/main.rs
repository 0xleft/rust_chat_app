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

#[derive(Serialize, Deserialize)]
pub struct ChatMessage {
    from: String,
    to: String,
    content: String,
}

type Users = Arc<RwLock<Vec<User>>>;
type ConnectedUsers = Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Message>>>>;

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

    // get /users -> all users
    let users_path = warp::path("users")
        .and(warp::get())
        .and(with_users(users.clone()))
        .map(|users: Users| {
            let users = users.try_read().unwrap();
            warp::reply::json(&*users)
        });

    let routes = register_path.or(users_path).or(chat);

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

async fn register_user(user: User, users: Users) -> Result<impl warp::Reply, warp::Rejection> {
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
    let (user_ws_tx, mut user_ws_rx) = ws.split();
    let (user_connected_tx, user_connected_rx) = mpsc::unbounded_channel::<Message>();

    print!("User connected {:?}", user_ws_tx);
}