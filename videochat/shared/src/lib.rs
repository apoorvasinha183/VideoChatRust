use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct IceCandidateData {
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_m_line_index: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SignalMessage {
    Offer(String),
    Answer(String),
    IceCandidate(IceCandidateData),
}
