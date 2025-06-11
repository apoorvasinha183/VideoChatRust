use web_sys::WebSocket;
pub use shared::{IceCandidateData, SignalMessage};

/// Helper to create a WebSocket from a URL.
pub fn create_websocket(url: &str) -> WebSocket {
    WebSocket::new(url).expect("Failed to create WebSocket")
}
