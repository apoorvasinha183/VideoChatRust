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
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
enum SignalMessage {
    Offer(String),       // SDP offer
    Answer(String),      // SDP answer
    IceCandidate(String) // ICE candidate information
}

#[function_component(App)]
fn app() -> Html {
    // NodeRefs for the local & remote <video> elements.
    let video_ref = use_node_ref();
    let remote_video_ref = use_node_ref();

    // State for the WebSocket connection and the PeerConnection.
    let ws_state = use_state(|| Option::<WebSocket>::None);
    let pc_state = use_state(|| Option::<RtcPeerConnection>::None);

    {
        // Clone state handles for the effect.
        let video_ref_clone = video_ref.clone();
        let remote_video_ref_clone = remote_video_ref.clone();
        let ws_state_inner = ws_state.clone();
        let pc_state_inner = pc_state.clone();

        use_effect_with_deps(
            move |_| {
                // 1. Set up local video stream.
                let win = window().expect("should have a window");
                let navigator = win.navigator();
                let media_devices = navigator.media_devices().expect("no media devices available");

                let constraints = MediaStreamConstraints::new();
                constraints.set_video(&JsValue::TRUE);
                let promise = media_devices
                    .get_user_media_with_constraints(&constraints)
                    .expect("getUserMedia should work");

                let local_stream_future = async move {
                    let stream_js = JsFuture::from(promise)
                        .await
                        .expect("failed to get media stream");
                    let stream: MediaStream = stream_js.dyn_into().unwrap();

                    // Attach local stream to local video element.
                    if let Some(video) = video_ref_clone.cast::<HtmlVideoElement>() {
                        // Use unchecked_ref() to match the expected type.
                        video.set_src_object(Some(stream.unchecked_ref()));
                        let _ = video.play();
                    }
                    stream
                };

                // 2. Set up the PeerConnection.
                {
                    let pc_state_async = pc_state_inner.clone();
                    let ws_state_async = ws_state_inner.clone();
                    let remote_video_for_pc = remote_video_ref_clone.clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        let local_stream = local_stream_future.await;
                        let config = RtcConfiguration::new();
                        let pc = RtcPeerConnection::new_with_configuration(&config)
                            .expect("Failed to create RTCPeerConnection");

                        // Add local tracks.
                        let tracks = local_stream.get_tracks();
                        for i in 0..tracks.length() {
                            let track_val = tracks.get(i);
                            if !track_val.is_undefined() {
                                let track: MediaStreamTrack = track_val.dyn_into().unwrap();
                                let empty_array = Array::new();
                                let _ = pc.add_track(&track, &local_stream, &empty_array);
                            }
                        }

                        // --- Set up ontrack handler for remote streams.
                        {
                            let remote_video_clone = remote_video_for_pc.clone();
                            let on_track = Closure::wrap(Box::new(move |evt: RtcTrackEvent| {
                                let stream_val = evt.streams().get(0);
                                if !stream_val.is_undefined() && !stream_val.is_null() {
                                    if let Some(video) = remote_video_clone.cast::<HtmlVideoElement>() {
                                        video.set_src_object(Some(stream_val.unchecked_ref()));
                                        let _ = video.play();
                                    }
                                }
                            }) as Box<dyn FnMut(_)>);
                            pc.set_ontrack(Some(on_track.as_ref().unchecked_ref()));
                            on_track.forget();
                        }
                        // --- End ontrack handler

                        // --- Initiate SDP offer (if this peer is the initiator).
                        {
                            let ws_for_offer = ws_state_async.clone();
                            let pc_clone = pc.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                let offer_js = JsFuture::from(pc_clone.create_offer())
                                    .await
                                    .expect("Offer creation failed");
                                let offer: RtcSessionDescriptionInit = offer_js.dyn_into().unwrap();
                                let set_ld = pc_clone.set_local_description(&offer);
                                let _ = JsFuture::from(set_ld)
                                    .await
                                    .expect("Error setting local description");
                                // Extract SDP string via Reflect.
                                let sdp_val = Reflect::get(&offer, &JsValue::from_str("sdp"))
                                    .unwrap()
                                    .as_string()
                                    .unwrap_or_default();
                                let msg = SignalMessage::Offer(sdp_val);
                                let msg_json = serde_json::to_string(&msg).unwrap();
                                if let Some(ws) = ws_for_offer.as_ref() {
                                    let _ = ws.send_with_str(&msg_json);
                                }
                            });
                        }
                        // --- End offer initiation

                        // ICE candidate handler.
                        {
                            let ws_for_ice = ws_state_async.clone();
                            let on_ice_candidate = Closure::wrap(Box::new(move |evt: RtcPeerConnectionIceEvent| {
                                if let Some(candidate) = evt.candidate() {
                                    let candidate_str = candidate.candidate();
                                    let msg = SignalMessage::IceCandidate(candidate_str);
                                    let msg_json = serde_json::to_string(&msg).unwrap();
                                    if let Some(ws) = ws_for_ice.as_ref() {
                                        let _ = ws.send_with_str(&msg_json);
                                    }
                                }
                            }) as Box<dyn FnMut(_)>);
                            pc.set_onicecandidate(Some(on_ice_candidate.as_ref().unchecked_ref()));
                            on_ice_candidate.forget();
                        }

                        // Store the PeerConnection.
                        pc_state_async.set(Some(pc));
                    });
                }

                // 3. Establish the WebSocket connection.
                {
                    let ws_for_ws = ws_state_inner.clone();
                    let ws_url = "ws://192.168.1.121:3030/ws"; // Use your local IP.
                    let ws = WebSocket::new(ws_url).expect("WebSocket creation failed");
                    ws_for_ws.set(Some(ws));
                }

                // 4. Set up the WebSocket message handler.
                {
                    // Clone the state handle once for use in this block.
                    let ws_for_msg = ws_state_inner.clone();
                    let pc_for_msg = pc_state_inner.clone();
                    // Create a clone for the closure (so the original remains available).
                    let ws_for_msg_inner = ws_for_msg.clone();
                    let on_message = Closure::wrap(Box::new(move |evt: MessageEvent| {
                        if let Some(txt) = evt.data().as_string() {
                            if let Ok(signal) = serde_json::from_str::<SignalMessage>(&txt) {
                                if let Some(pc) = pc_for_msg.as_ref() {
                                    match signal {
                                        SignalMessage::Offer(sdp_str) => {
                                            let mut offer_desc = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                                            offer_desc.set_sdp(&sdp_str);
                                            let pc_for_offer = pc_for_msg.clone();
                                            let ws_for_offer = ws_for_msg_inner.clone();
                                            wasm_bindgen_futures::spawn_local(async move {
                                                if let Some(pc) = pc_for_offer.as_ref() {
                                                    let set_rd_prom = pc.set_remote_description(&offer_desc);
                                                    let _ = JsFuture::from(set_rd_prom)
                                                        .await
                                                        .expect("Error setting remote description");
                                                    let ans_js = JsFuture::from(pc.create_answer())
                                                        .await
                                                        .expect("Error creating answer");
                                                    let answer: RtcSessionDescriptionInit = ans_js.dyn_into().unwrap();
                                                    let set_ld = pc.set_local_description(&answer);
                                                    let _ = JsFuture::from(set_ld)
                                                        .await
                                                        .expect("Error setting local description for answer");
                                                    let sdp_val = Reflect::get(&answer, &JsValue::from_str("sdp"))
                                                        .unwrap()
                                                        .as_string()
                                                        .unwrap_or_default();
                                                    web_sys::console::log_1(&format!("Created answer: {}", sdp_val).into());
                                                    let msg = SignalMessage::Answer(sdp_val);
                                                    let msg_json = serde_json::to_string(&msg).unwrap();
                                                    if let Some(ws) = ws_for_offer.as_ref() {
                                                        let _ = ws.send_with_str(&msg_json);
                                                    }
                                                }
                                            });
                                        },
                                        SignalMessage::Answer(sdp_str) => {
                                            let mut ans_desc = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                                            ans_desc.set_sdp(&sdp_str);
                                            let pc_for_ans = pc_for_msg.clone();
                                            wasm_bindgen_futures::spawn_local(async move {
                                                if let Some(pc) = pc_for_ans.as_ref() {
                                                    let set_rd = pc.set_remote_description(&ans_desc);
                                                    let _ = JsFuture::from(set_rd)
                                                        .await
                                                        .expect("Error setting remote desc for answer");
                                                }
                                            });
                                        },
                                        SignalMessage::IceCandidate(cand_str) => {
                                            let cand_init = RtcIceCandidateInit::new(&cand_str);
                                            if let Ok(cand) = RtcIceCandidate::new(&cand_init) {
                                                if let Some(pc) = pc_for_msg.as_ref() {
                                                    let _ = pc.add_ice_candidate_with_opt_rtc_ice_candidate(Some(&cand));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }) as Box<dyn FnMut(_)>);
                    if let Some(ws) = ws_for_msg.as_ref() {
                        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
                    }
                    on_message.forget();
                }

                || ()
            },
            // Depend on video_ref so the effect re-runs when it updates.
            video_ref.clone()
        );
    }

    html! {
        <div>
            <h1>{ "Rust Video Chat (Enhanced)" }</h1>
            <video ref={video_ref} autoplay=true playsinline=true muted=true
                   style="width: 480px; height: 360px; background: #000;" />
            <video ref={remote_video_ref} autoplay=true playsinline=true
                   style="width: 480px; height: 360px; background: #000;" />
            <p>{ "This is the enhanced version with WebSocket signaling and WebRTC integration." }</p>
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn run_app() {
    yew::Renderer::<App>::new().render();
}
