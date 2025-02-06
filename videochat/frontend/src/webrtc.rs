use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    RtcPeerConnection, RtcConfiguration, RtcPeerConnectionIceEvent, RtcTrackEvent,
    HtmlVideoElement, WebSocket, RtcSessionDescriptionInit, RtcSdpType, MediaStream,
};
use js_sys::{Array, Reflect};
use std::rc::Rc;
use std::cell::RefCell;
use crate::signaling::{SignalMessage, IceCandidateData};

/// Creates and returns a new RTCPeerConnection with a basic configuration.
pub fn create_peer_connection() -> RtcPeerConnection {
    let config = {
        let config = RtcConfiguration::new();
        let ice_server = {
            let server = web_sys::RtcIceServer::new();
            server.set_urls(&JsValue::from_str("stun:stun.l.google.com:19302"));
            server.set_credential("");
            server.set_username("");
            server
        };
        let ice_servers = Array::new();
        ice_servers.push(&ice_server);
        config.set_ice_servers(&ice_servers);
        config.set_ice_transport_policy(web_sys::RtcIceTransportPolicy::All);
        config
    };
    RtcPeerConnection::new_with_configuration(&config)
        .expect("Failed to create RTCPeerConnection")
}

/// Attaches common event handlers (ICE state change, ontrack, onicecandidate) to the given peer connection.
pub fn attach_event_handlers(
    pc: &RtcPeerConnection,
    remote_video: HtmlVideoElement,
    ws_ref: Rc<RefCell<Option<WebSocket>>>,
    ice_candidate_queue: Rc<RefCell<Vec<String>>>,
) {
    // Attach ICE connection state change handler.
    {
        let pc_clone = pc.clone();
        let on_ice_state_change = Closure::wrap(Box::new(move || {
            web_sys::console::log_1(
                &format!("ICE connection state: {:?}", pc_clone.ice_connection_state()).into(),
            );
        }) as Box<dyn FnMut()>);
        pc.set_oniceconnectionstatechange(Some(on_ice_state_change.as_ref().unchecked_ref()));
        on_ice_state_change.forget();
    }

    // Attach ontrack handler.
    {
        let on_track = Closure::wrap(Box::new(move |evt: RtcTrackEvent| {
            web_sys::console::log_1(&"Remote track event triggered".into());
            let track = evt.track();
            web_sys::console::log_1(&format!("Remote track kind: {}", track.kind()).into());
            web_sys::console::log_1(&format!("Remote track id: {}", track.id()).into());
            let streams = evt.streams();
            web_sys::console::log_1(&format!("Number of streams in event: {}", streams.length()).into());
            let stream_val = streams.get(0);
            if stream_val.is_undefined() || stream_val.is_null() {
                web_sys::console::log_1(&"No stream provided with ontrack; creating new MediaStream".into());
                let new_stream = web_sys::MediaStream::new().expect("Failed to create MediaStream");
                new_stream.add_track(&evt.track());
                remote_video.set_src_object(Some(new_stream.unchecked_ref()));
                let play_result = remote_video.play();
                web_sys::console::log_1(&format!("Attempted video.play() with new stream, result: {:?}", play_result).into());
            } else {
                web_sys::console::log_1(&"Using provided stream from ontrack event".into());
                remote_video.set_src_object(Some(stream_val.unchecked_ref()));
                let play_result = remote_video.play();
                web_sys::console::log_1(&format!("Attempted video.play() with provided stream, result: {:?}", play_result).into());
            }
        }) as Box<dyn FnMut(_)>);
        pc.set_ontrack(Some(on_track.as_ref().unchecked_ref()));
        on_track.forget();
    }

    // Attach onicecandidate handler.
    {
        let ws_ref_clone = ws_ref.clone();
        let ice_candidate_queue_clone = ice_candidate_queue.clone();
        let on_ice_candidate = Closure::wrap(Box::new(move |evt: RtcPeerConnectionIceEvent| {
            if let Some(candidate) = evt.candidate() {
                web_sys::console::log_1(&format!("Sending ICE candidate: {}", candidate.candidate()).into());
                let data = IceCandidateData {
                    candidate: candidate.candidate(),
                    sdp_mid: candidate.sdp_mid(),
                    sdp_m_line_index: candidate.sdp_m_line_index(),
                };
                let msg = SignalMessage::IceCandidate(data);
                let msg_json = serde_json::to_string(&msg).unwrap();
                if let Some(ws) = ws_ref_clone.borrow().as_ref() {
                    if ws.ready_state() == web_sys::WebSocket::OPEN {
                        let _ = ws.send_with_str(&msg_json);
                        web_sys::console::log_1(&"WebSocket Ready!!!!".into());
                    } else {
                        ice_candidate_queue_clone.borrow_mut().push(msg_json);
                        web_sys::console::log_1(&"WebSocket not ready yet; candidate queued.".into());
                    }
                } else {
                    web_sys::console::log_1(&"WebSocket not ready yet; candidate queued.".into());
                    ice_candidate_queue_clone.borrow_mut().push(msg_json);
                }
            }
        }) as Box<dyn FnMut(_)>);
        pc.set_onicecandidate(Some(on_ice_candidate.as_ref().unchecked_ref()));
        on_ice_candidate.forget();
    }
}

/// Attaches WebSocket event handlers (onopen, onerror, onclose, onmessage) to the given WebSocket.
/// Flushes any queued ICE candidates when the socket opens.
pub fn attach_websocket_handlers(
    ws: &WebSocket,
    ws_ref: Rc<RefCell<Option<WebSocket>>>,
    ice_candidate_queue: Rc<RefCell<Vec<String>>>,
    _pc_ref: Rc<RefCell<Option<RtcPeerConnection>>>,
) {
    // onopen: flush queued ICE candidates.
    let ws_ref_for_onopen = ws_ref.clone();
    let ice_candidate_queue_clone = ice_candidate_queue.clone();
    let on_open = Closure::wrap(Box::new(move |_| {
        web_sys::console::log_1(&"WebSocket connection opened!".into());
        let mut queue = ice_candidate_queue_clone.borrow_mut();
        if let Some(ws) = ws_ref_for_onopen.borrow().as_ref() {
            for candidate_json in queue.drain(..) {
                ws.send_with_str(&candidate_json).ok();
            }
        }
    }) as Box<dyn FnMut(JsValue)>);
    ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    on_open.forget();

    // onerror: log errors.
    let on_error = Closure::wrap(Box::new(move |e: JsValue| {
        web_sys::console::error_1(&format!("WebSocket error: {:?}", e).into());
    }) as Box<dyn FnMut(JsValue)>);
    ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    on_error.forget();

    // onclose: log when the WebSocket closes.
    let on_close = Closure::wrap(Box::new(move |_| {
        web_sys::console::log_1(&"WebSocket connection closed!".into());
    }) as Box<dyn FnMut(JsValue)>);
    ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
    on_close.forget();

    // onmessage: log received messages (further parsing can be added here).
    let on_message = Closure::wrap(Box::new(move |evt: web_sys::MessageEvent| {
        if let Some(txt) = evt.data().as_string() {
            web_sys::console::log_1(&format!("Received raw message as text: {}", txt).into());
            // Here, you would parse and handle the message (offer, answer, ICE candidate).
        } else {
            web_sys::console::log_1(&format!("Received non-text message: {:?}", evt.data()).into());
        }
    }) as Box<dyn FnMut(_)>);
    ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    on_message.forget();
}
