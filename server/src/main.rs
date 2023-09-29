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
type ConnectedUsers = Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Message>>>>;

#[tokio::main]
async fn main() {
    let users: Users = Users::default();

    // post /register
    let register = warp::path("register")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_users(users))
        .and_then(register_user);

    // get /users -> all users
    let users = warp::path("users")
        .and(warp::get())
        .and(with_users(cloned_users))
        .map(|users: Users| async move {
            let out_users = users.read().await;
            warp::reply::json(&*out_users)
        });

    let routes = register;

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