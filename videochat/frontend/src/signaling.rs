use serde::{Serialize, Deserialize};
use web_sys::WebSocket;

#[derive(Serialize, Deserialize)]
pub struct IceCandidateData {
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_m_line_index: Option<u16>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SignalMessage {
    Offer(String),
    Answer(String),
    IceCandidate(IceCandidateData),
}

/// Helper to create a WebSocket from a URL.
pub fn create_websocket(url: &str) -> WebSocket {
    WebSocket::new(url).expect("Failed to create WebSocket")
}
