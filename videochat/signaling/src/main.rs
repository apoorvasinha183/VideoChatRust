use std::sync::{Arc, Mutex};
use futures::{SinkExt, StreamExt};
use warp::ws::{Message, WebSocket};
use warp::Filter;
use tokio::sync::mpsc;

type Clients = Arc<Mutex<Vec<tokio::sync::mpsc::UnboundedSender<Message>>>>;

#[tokio::main]
async fn main() {
    // Shared state to track connected clients.
    let clients: Clients = Arc::new(Mutex::new(Vec::new()));
    let clients_filter = warp::any().map(move || clients.clone());

    // WebSocket route at /ws
    let ws_route = warp::path("ws")
        .and(warp::ws())
        .and(clients_filter)
        .map(|ws: warp::ws::Ws, clients| {
            ws.on_upgrade(move |socket| handle_connection(socket, clients))
        });

    println!("Signaling server running on 0.0.0.0:3030");
    warp::serve(ws_route).run(([0, 0, 0, 0], 3030)).await;
}

async fn handle_connection(ws: WebSocket, clients: Clients) {
    // Split the socket into a sender (tx) and receiver (rx)
    let (mut ws_tx, mut ws_rx) = ws.split();

    // Create a channel to forward messages to this client.
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Add the sender to the list of clients.
    {
        let mut clients_lock = clients.lock().unwrap();
        clients_lock.push(tx);
    }

    // Spawn a task to forward messages from the rx channel to the WebSocket.
    let forward_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Process incoming WebSocket messages and broadcast them.
    while let Some(result) = ws_rx.next().await {
        match result {
            Ok(msg) => {
                broadcast_message(msg, &clients).await;
            }
            Err(e) => {
                eprintln!("websocket error: {}", e);
                break;
            }
        }
    }

    // Remove disconnected clients.
    {
        let mut clients_lock = clients.lock().unwrap();
        clients_lock.retain(|sender| !sender.is_closed());
    }

    forward_task.await.unwrap();
}

async fn broadcast_message(msg: Message, clients: &Clients) {
    let clients_lock = clients.lock().unwrap();
    for tx in clients_lock.iter() {
        // Send a cloned copy of the message to each client.
        let _ = tx.send(msg.clone());
    }
}
