use std::sync::{Arc, Mutex};
use futures::{SinkExt, StreamExt};
use warp::ws::{Message, WebSocket};
use warp::Filter;
use tokio::sync::mpsc;
use serde_json::json;
use rand::random;
use shared::{SignalMessage};
//type Clients = Arc<Mutex<Vec<tokio::sync::mpsc::UnboundedSender<Message>>>>;
type ClientId = usize;
type Clients = Arc<Mutex<Vec<(ClientId, tokio::sync::mpsc::UnboundedSender<Message>)>>>;
type Visitors = Arc<Mutex<Vec<tokio::sync::mpsc::UnboundedSender<Message>>>>;
#[tokio::main]
async fn main() {
    // Shared state to track connected clients.
    let clients: Clients = Arc::new(Mutex::new(Vec::new()));
    let clients_filter = warp::any().map(move || clients.clone());
    // visitor count
    // visitor connections: a list of sender channels for each visitor.
    let visitors: Visitors = Arc::new(Mutex::new(Vec::new()));
    let visitors_filter = warp::any().map(move || visitors.clone());

    // WebSocket route at /ws
    let ws_route = warp::path("ws")
        .and(warp::ws())
        .and(clients_filter)
        .map(|ws: warp::ws::Ws, clients| {
            ws.on_upgrade(move |socket| handle_connection(socket, clients))
        });
    let visitors_route = warp::path("visitors")
        .and(warp::ws())
        .and(visitors_filter)
        .map(|ws: warp::ws::Ws, visitors: Visitors| {
            ws.on_upgrade(move |socket| handle_visitor_connection(socket, visitors))
        });
    

    println!("Signaling server running on 0.0.0.0:3030");
    //warp::serve(ws_route).run(([0, 0, 0, 0], 3030)).await;
    let routes = ws_route.or(visitors_route);
    println!("Signaling server running on 0.0.0.0:3030");
    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}

async fn handle_connection(ws: WebSocket, clients: Clients) {
    // Split the socket into a sender (tx) and receiver (rx)
    let (mut ws_tx, mut ws_rx) = ws.split();
    println!("New signaling connection established");


    // Create a channel to forward messages to this client.
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Add the sender to the list of clients.
    let client_id = random::<usize>(); // generate a random client ID
    {
        let mut clients_lock = clients.lock().unwrap();
        clients_lock.push((client_id, tx));
        println!("Added new signaling client with id {}. Total clients: {}", client_id, clients_lock.len());
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
                if let Ok(text) = msg.to_str() {
                    if let Ok(parsed) = serde_json::from_str::<SignalMessage>(text) {
                        println!("Received message from {}: {:?}", client_id, parsed);
                    } else {
                        println!("Received non-signal text from {}: {}", client_id, text);
                    }
                }
                broadcast_message(msg, &clients, client_id).await;
            }
            Err(e) => {
                eprintln!("WebSocket error in signaling connection: {}", e);
                break;
            }
        }
    }
    

    // Remove disconnected clients.
    {
        let mut clients_lock = clients.lock().unwrap();
        //clients_lock.retain(|sender| !sender.is_closed());
        clients_lock.retain(|(_client_id, tx)| !tx.is_closed());
        println!("Cleaned up signaling clients. Remaining: {}", clients_lock.len());
    }

    forward_task.await.unwrap();
}

async fn broadcast_message(msg: Message, clients: &Clients, sender_id: ClientId) {
    let clients_lock = clients.lock().unwrap();
    println!(
        "Broadcasting message from sender {} to {} clients",
        sender_id,
        clients_lock.len()
    );

    if let Ok(text) = msg.to_str() {
        if let Ok(parsed) = serde_json::from_str::<SignalMessage>(text) {
            println!("Parsed SignalMessage: {:?}", parsed);
        }
    }

    for (client_id, tx) in clients_lock.iter() {
        if *client_id != sender_id {
            let _ = tx.send(msg.clone());
            println!("Sent message to client id {}", client_id);
        } else {
            println!("Skipping sender id {}", sender_id);
        }
    }
}



async fn handle_visitor_connection(ws: WebSocket, visitors: Visitors) {
    // Split the WebSocket into sender (ws_tx) and receiver (ws_rx)
    let (mut ws_tx, mut ws_rx) = ws.split();
    println!("New visitor connection established");


    // Create a channel for sending messages to this visitor.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    // Add this visitor's sender to the global visitors list.
    {
        let mut visitors_lock = visitors.lock().unwrap();
        visitors_lock.push(tx);
        println!("Added new visitor. Total visitors: {}", visitors_lock.len());
    }

    // Immediately broadcast the updated visitor count to all visitors.
    broadcast_visitor_count(&visitors).await;

    // Spawn a task to forward messages from the rx channel to the WebSocket.
    let forward_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Process incoming messages (if any) until the connection closes.
    while let Some(result) = ws_rx.next().await {
        if result.is_err() {
            break;
        }
        // In this example, we don't process visitor messages.
    }

    // When the connection closes, remove this visitor's sender.
    {
        let mut visitors_lock = visitors.lock().unwrap();
        visitors_lock.retain(|sender| !sender.is_closed());
    }
    // After removal, broadcast the updated count.
    broadcast_visitor_count(&visitors).await;

    // Wait for the forward task to finish.
    let _ = forward_task.await;
}

async fn broadcast_visitor_count(visitors: &Visitors) {
    // Get the current count.
    let count = {
        let visitors_lock = visitors.lock().unwrap();
        visitors_lock.len()
    };

    // Build the JSON message.
    let msg = Message::text(serde_json::json!({ "visitorCount": count }).to_string());

    // Send the message to every connected visitor.
    let visitors_lock = visitors.lock().unwrap();
    for sender in visitors_lock.iter() {
        let _ = sender.send(msg.clone());
    }
}

