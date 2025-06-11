use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlVideoElement, MediaStream, MediaStreamConstraints, window, WebSocket,
    RtcPeerConnection, RtcConfiguration, RtcPeerConnectionIceEvent, RtcIceCandidateInit,
    MessageEvent, MediaStreamTrack, RtcSessionDescriptionInit, RtcSdpType, RtcTrackEvent,
    RtcIceCandidate,
};
use yew::prelude::*;
use wasm_bindgen_futures::JsFuture;
use serde_json;
use js_sys::{Array, Reflect};

use std::rc::Rc;
use std::cell::RefCell;
mod visitor_counter;
mod signaling;
use visitor_counter::VisitorCounter;
use web_sys::MediaTrackConstraints;
use signaling::{IceCandidateData, SignalMessage};

#[function_component(App)]
fn app() -> Html {
    // Reference to the local video element (displaying our own stream)
    let video_ref = use_node_ref();
    // Reference to the remote video element (displaying the remote stream)
    let remote_video_ref = use_node_ref();
    
    // Use a mutable reference to store the WebSocket so that closures can always access the latest instance.
    // (Replaces a use_state approach which can capture stale values.)
    let ws_ref = use_mut_ref(|| Option::<WebSocket>::None); // <<-- CHANGED

    // Store the RTCPeerConnection instance in a mutable reference
    let pc_ref = use_mut_ref::<Option<RtcPeerConnection>, _>(|| None);
    // A queue for ICE candidates that are generated before the WebSocket is ready.
    let ice_candidate_queue = Rc::new(RefCell::new(Vec::<String>::new()));
    // A state to ensure the call is initiated only once.
    let offer_sent = use_state(|| false);
    let local_track_ids = Rc::new(RefCell::new(Vec::<String>::new()));  
    let offer_sent_for_button = offer_sent.clone();

    // Callback for when the "Start Call" button is pressed.
    let on_start_call = {
        let pc_ref = pc_ref.clone();
        // Capture the WebSocket mutable reference so that we always use the current value.
        let ws_ref_clone = ws_ref.clone(); // <<-- CHANGED
        let offer_sent_for_call = offer_sent.clone();
        
        Callback::from(move |_| {
            web_sys::console::log_1(&"Start Call button pressed".into());
            if !*offer_sent_for_call {
                offer_sent_for_call.set(true);
                
                // Retrieve the RTCPeerConnection instance, if available.
                if let Some(pc) = pc_ref.borrow().clone() {
                    let pc_clone = pc.clone();
                    // Get the current WebSocket instance from ws_ref.
                    let ws_clone = (*ws_ref_clone.borrow()).clone(); // <<-- CHANGED
                    
                    wasm_bindgen_futures::spawn_local(async move {
                        // Create an SDP offer
                        let offer_js = JsFuture::from(pc_clone.create_offer())
                            .await
                            .expect("Offer creation failed");

                        // Extract the SDP string from the offer object.
                        let sdp = Reflect::get(&offer_js, &JsValue::from_str("sdp"))
                            .expect("No sdp field")
                            .as_string()
                            .expect("sdp field is not a string");

                        web_sys::console::log_1(&format!("Created offer with sdp: {}", sdp).into());
                        let offer = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                        offer.set_sdp(&sdp);

                        // Set the local description with the created offer.
                        let set_ld = pc_clone.set_local_description(&offer);
                        let _ = JsFuture::from(set_ld).await;

                        // If the WebSocket is available, send the offer message.
                        if let Some(ws) = ws_clone.as_ref() {
                            let sdp = Reflect::get(&offer, &JsValue::from_str("sdp"))
                                .unwrap()
                                .as_string()
                                .unwrap();
                            let msg = SignalMessage::Offer(sdp);
                            let msg_json = serde_json::to_string(&msg).unwrap();
                            web_sys::console::log_1(&format!("Sending Offer message: {}", msg_json).into());
                            ws.send_with_str(&msg_json).ok();
                        }
                    });
                }
            }
        })
    };

    {
        // Clone references for use in the effect.
        let video_ref_clone = video_ref.clone();
        let remote_video_ref_clone = remote_video_ref.clone();
        // Use ws_ref instead of a state so that we always access the current WebSocket instance.
        let ws_ref_inner = ws_ref.clone(); // <<-- CHANGED
        let pc_ref_inner = pc_ref.clone();
    
        use_effect_with_deps(
            move |_| {
                // Create the configuration for the RTCPeerConnection.
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

                // Create the RTCPeerConnection.
                let pc = RtcPeerConnection::new_with_configuration(&config)
                    .expect("Failed to create RTCPeerConnection");
                
                // >> DEBUG: Log ICE connection state changes.
                {
                    let pc_clone = pc.clone();
                    let on_ice_state_change = Closure::wrap(Box::new(move || {
                        // Log the ICE connection state using the native method.
                        // (Depending on your web-sys version, this may need adjustment.)
                        web_sys::console::log_1(&format!("ICE connection state: {:?}", pc_clone.ice_connection_state()).into());
                    }) as Box<dyn FnMut()>);
                    pc.set_oniceconnectionstatechange(Some(on_ice_state_change.as_ref().unchecked_ref()));
                    on_ice_state_change.forget();
                }
                // << END DEBUG

                // Attach the ontrack handler to process remote media tracks.
                {
                    let remote_video_clone = remote_video_ref_clone.clone();
                    let local_track_ids_for_ontrack = local_track_ids.clone();
                    let on_track = Closure::wrap(Box::new(move |evt: RtcTrackEvent| {
                        let incoming_id = evt.track().id();
                        if local_track_ids_for_ontrack.borrow().contains(&incoming_id) {
                            web_sys::console::log_1(&format!("Ignoring local track: {}", incoming_id).into());
                            return;
                        }
                        web_sys::console::log_1(&"Remote track event triggered".into());
                        let track = evt.track();
                        web_sys::console::log_1(&format!("Remote track kind: {}", track.kind()).into());
                        web_sys::console::log_1(&format!("Remote track id: {}", track.id()).into());
                        let streams = evt.streams();
                        web_sys::console::log_1(&format!("Number of streams in event: {}", streams.length()).into());

                        // Use the provided stream if available; otherwise, create a new MediaStream.
                        let stream_val = streams.get(0);
                        if stream_val.is_undefined() || stream_val.is_null() {
                            web_sys::console::log_1(&"No stream provided with ontrack; creating new MediaStream".into());
                            let new_stream = web_sys::MediaStream::new().expect("Failed to create MediaStream");
                            new_stream.add_track(&evt.track());
                            if let Some(video) = remote_video_clone.cast::<HtmlVideoElement>() {
                                video.set_src_object(Some(new_stream.unchecked_ref()));
                                let play_result = video.play();
                                web_sys::console::log_1(&format!("Attempted video.play() with new stream, result: {:?}", play_result).into());
                            }
                        } else {
                            web_sys::console::log_1(&"Using provided stream from ontrack event".into());
                            if let Some(video) = remote_video_clone.cast::<HtmlVideoElement>() {
                                video.set_src_object(Some(stream_val.unchecked_ref()));
                                let play_result = video.play();
                                web_sys::console::log_1(&format!("Attempted video.play() with provided stream, result: {:?}", play_result).into());
                            }
                        }
                    }) as Box<dyn FnMut(_)>);
                    pc.set_ontrack(Some(on_track.as_ref().unchecked_ref()));
                    on_track.forget();
                }
    
                // Attach the onicecandidate handler to send ICE candidates via the WebSocket.
                {
                    let pc_for_ice = pc.clone();
                    let local_track_ids_for_async = local_track_ids.clone();
                    // Use the latest WebSocket instance via ws_ref.
                    let ws_ref_clone = ws_ref_inner.clone(); // <<-- CHANGED
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
                            // Always retrieve the current WebSocket instance.
                            if let Some(ws) = ws_ref_clone.borrow().as_ref() { // <<-- CHANGED here
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
                    pc_for_ice.set_onicecandidate(Some(on_ice_candidate.as_ref().unchecked_ref()));
                    on_ice_candidate.forget();
                }
    
                // Store the peer connection in our mutable reference.
                *pc_ref_inner.borrow_mut() = Some(pc.clone());
                if (*pc_ref_inner.borrow()).is_none() {
                    web_sys::console::log_1(&"pc_ref_inner is empty".into());
                } else {
                    web_sys::console::log_1(&"pc_ref_inner is not empty".into());
                }
                web_sys::console::log_1(&"!!!! Just set pc_ref_inner to Some(pc) !!!!".into());
    
                // Obtain the user media (camera stream) asynchronously and attach it to the local video element.
                {
                    let navigator = window().unwrap().navigator();
                    let media_devices = navigator.media_devices().expect("no media devices available");
    
                    let constraints = MediaStreamConstraints::new();
                    constraints.set_video(&JsValue::TRUE);
                    // Echo cancellation maybe
                    //constraints.set_audio(&JsValue::TRUE);  // BRUH BRUH BRUH 
                    let mut audio_constraints = MediaTrackConstraints::new();
                    audio_constraints.echo_cancellation(&JsValue::from(true));
                    // For Chrome-based browsers
                    audio_constraints.echo_cancellation(&JsValue::from(true));  
                    constraints.set_audio(&audio_constraints.into());
                    let promise = media_devices
                        .get_user_media_with_constraints(&constraints)
                        .expect("getUserMedia should work");
    
                    let video_for_async = video_ref_clone.clone();
                    let pc_for_tracks = pc.clone();
                    let local_track_ids_for_async = local_track_ids.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        let stream_js = JsFuture::from(promise).await.expect("failed to get media stream");
                        let stream: MediaStream = stream_js.dyn_into().unwrap();
    
                        // DEBUG: Log that the local media stream was successfully acquired.
                        web_sys::console::log_1(&"Got local media stream".into());
    
                        if let Some(video) = video_for_async.cast::<HtmlVideoElement>() {
                            video.set_src_object(Some(stream.unchecked_ref()));
                            let _ = video.play();
                        }
    
                        // Add each track of the stream to the peer connection.
                        let tracks = stream.get_tracks();
                        for i in 0..tracks.length() {
                            let track_val = tracks.get(i);
                            if !track_val.is_undefined() {
                                let track: MediaStreamTrack = track_val.dyn_into().unwrap();
                                local_track_ids_for_async.borrow_mut().push(track.id());
                                let empty_array = Array::new();
                                let _ = pc_for_tracks.add_track(&track, &stream, &empty_array);
                            }
                        }
                    });
                }
    
                // Create the WebSocket connection only after the peer connection is set.
                {
                    // Use ws_ref instead of a state for storing the WebSocket.
                    let ws_ref_for_state = ws_ref_inner.clone(); // <<-- CHANGED
                    let offer_sent_for_msg = offer_sent.clone();
                    let ws_host = window().unwrap().location().host().unwrap();
                    let ws_url  = format!("wss://{}/ws", ws_host);
    
                    let ws = WebSocket::new(&ws_url).expect("WebSocket creation failed");
                    // Use ws_ref to ensure that the onopen handler uses the current WebSocket.
                    let ws_ref_for_onopen = ws_ref_inner.clone(); // <<-- CHANGED
                    let ice_candidate_queue_clone2 = ice_candidate_queue.clone();
                    let on_open = Closure::wrap(Box::new(move |_| {
                        web_sys::console::log_1(&"WebSocket connection opened!".into());
                        let mut queue = ice_candidate_queue_clone2.borrow_mut();
                        if let Some(ws) = ws_ref_for_onopen.borrow().as_ref() { // <<-- CHANGED
                            // Flush any queued ICE candidates now that the socket is open.
                            for candidate_json in queue.drain(..) {
                                ws.send_with_str(&candidate_json).ok();
                            }
                        }
                    }) as Box<dyn FnMut(JsValue)>);
                    ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
                    on_open.forget();

                    // Log any WebSocket errors.
                    let on_error = Closure::wrap(Box::new(move |e: JsValue| {
                        web_sys::console::error_1(&format!("WebSocket error: {:?}", e).into());
                    }) as Box<dyn FnMut(JsValue)>);
                    ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));
                    on_error.forget();
    
                    // Log when the WebSocket connection is closed.
                    let on_close = Closure::wrap(Box::new(move |_| {
                        web_sys::console::log_1(&"WebSocket connection closed!".into());
                    }) as Box<dyn FnMut(JsValue)>);
                    ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
                    on_close.forget();
                    
                    // Use ws_ref for subsequent message handling.
                    let ws_for_closure = ws_ref_inner.clone(); // <<-- CHANGED
                    let on_message = {
                        let pc_ref_for_msg = pc_ref_inner.clone();
                        Closure::wrap(Box::new(move |evt: MessageEvent| {
                            if let Some(txt) = evt.data().as_string() {
                                web_sys::console::log_1(&format!("Received raw message as text: {}", txt).into());
                                if (*pc_ref_for_msg.borrow()).is_none() {
                                    web_sys::console::log_1(&"pc_ref_for_msg is empty".into());
                                } else {
                                    web_sys::console::log_1(&"pc_ref_for_msg is not empty".into());
                                }
                                match serde_json::from_str::<SignalMessage>(&txt) {
                                    Ok(signal) => {
                                        let maybe_pc = pc_ref_for_msg.borrow().clone();
                                        if let Some(pc) = maybe_pc {
                                            match signal {
                                                SignalMessage::Offer(sdp_str) => {
                                                    web_sys::console::log_1(&"Received Offer signal. Disabling Start Call on this client.".into());
                                                    offer_sent_for_msg.set(true);
    
                                                    let offer_desc = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                                                    offer_desc.set_sdp(&sdp_str);
    
                                                    let pc_for_async = pc.clone();
                                                    let ws_clone = ws_for_closure.clone();
    
                                                    wasm_bindgen_futures::spawn_local(async move {
                                                        let set_rd_prom = pc_for_async.set_remote_description(&offer_desc);
                                                        let _ = JsFuture::from(set_rd_prom).await;
                                                    
                                                        let ans_js = JsFuture::from(pc_for_async.create_answer())
                                                            .await
                                                            .expect("Error creating answer");
                                                    
                                                        let sdp = Reflect::get(&ans_js, &JsValue::from_str("sdp"))
                                                            .expect("No sdp field in answer")
                                                            .as_string()
                                                            .expect("sdp field is not a string");
                                                    
                                                        let answer = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                                                        answer.set_sdp(&sdp);
                                                    
                                                        let set_ld = pc_for_async.set_local_description(&answer);
                                                        let _ = JsFuture::from(set_ld).await;
                                                    
                                                        let msg = SignalMessage::Answer(sdp);
                                                        let msg_json = serde_json::to_string(&msg).unwrap();
                                                    
                                                        if let Some(ws) = ws_clone.borrow().as_ref() { // <<-- CHANGED
                                                            let _ = ws.send_with_str(&msg_json);
                                                        }
                                                    });
                                                },
    
                                                SignalMessage::Answer(sdp_str) => {
                                                    let ans_desc = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                                                    ans_desc.set_sdp(&sdp_str);
    
                                                    let pc_for_async = pc.clone();
                                                    wasm_bindgen_futures::spawn_local(async move {
                                                        let set_rd = pc_for_async.set_remote_description(&ans_desc);
                                                        let _ = JsFuture::from(set_rd).await;
                                                    });
                                                },
    
                                                SignalMessage::IceCandidate(data) => {
                                                    let cand_init = RtcIceCandidateInit::new(data.candidate.as_str());
                                                    if let Some(sdp_mid) = data.sdp_mid {
                                                        cand_init.set_sdp_mid(Some(&sdp_mid));
                                                    }
                                                    if let Some(sdp_ml_idx) = data.sdp_m_line_index {
                                                        cand_init.set_sdp_m_line_index(Some(sdp_ml_idx));
                                                    }
    
                                                    if let Ok(candidate) = RtcIceCandidate::new(&cand_init) {
                                                        let _ = pc.add_ice_candidate_with_opt_rtc_ice_candidate(Some(&candidate));
                                                    }
                                                },
                                            }
                                        } else {
                                            web_sys::console::log_1(&"Parsed SignalMessage OK, but 'pc_ref_for_msg' is None. Skipping signal.".into());
                                        }
                                    }
                                    Err(e) => {
                                        web_sys::console::log_1(&format!(
                                            "Failed to parse into SignalMessage: {}. Original text: {}",
                                            e, txt
                                        ).into());
                                    }
                                }
                            } else {
                                web_sys::console::log_1(&format!("Received non-text message: {:?}", evt.data()).into());
                            }
                        }) as Box<dyn FnMut(_)>)
                    };
    
                    ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
                    on_message.forget();
    
                    // Finally, store the WebSocket instance in our ws_ref mutable reference.
                    ws_ref_for_state.borrow_mut().replace(ws); // <<-- CHANGED
                }
    
                || ()
            },
            video_ref.clone()
        );
    }
    
    html! {
        <div>
            <h1>{ "Rust Video Chat" }</h1>
            <div style="display: flex; gap: 20px;">
                <video ref={video_ref} autoplay=true playsinline=true muted=true
                    style="width: 480px; height: 360px; background: #000;" />
                <video ref={remote_video_ref} autoplay=true playsinline=true muted=false
                    style="width: 480px; height: 360px; background: #000;" />
            </div>
            <button 
                onclick={on_start_call} 
                disabled={*offer_sent_for_button}
                style="margin-top: 20px; padding: 10px 20px;"
            >
                { "Start Call" }
            </button>
            <p>{ format!("Call initiated? {}", *offer_sent_for_button) }</p>
            <p>{ "Click 'Start Call' to initiate video chat" }</p>
            <VisitorCounter />
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn run_app() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
}
