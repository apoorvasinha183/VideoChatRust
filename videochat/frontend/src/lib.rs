use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlVideoElement, MediaStream, MediaStreamConstraints, window, WebSocket,
    RtcPeerConnection, RtcConfiguration, RtcPeerConnectionIceEvent, RtcIceCandidateInit,
    MessageEvent, MediaStreamTrack
};
use yew::prelude::*;
use wasm_bindgen_futures::JsFuture;
use serde_json;
use js_sys::Array;
use serde::{Serialize, Deserialize};
use gloo_timers::future::TimeoutFuture;
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
enum SignalMessage {
    Offer(String),       // SDP offer
    Answer(String),      // SDP answer
    IceCandidate(String) // ICE candidate information
}

#[function_component(App)]
fn app() -> Html {
    //let video_ref = NodeRef::default(); //Bruh it took me 6 hours to fix this!!
    let video_ref = use_node_ref();
    // State for storing the WebSocket connection and PeerConnection
    let ws_state = use_state(|| Option::<WebSocket>::None);
    let pc_state = use_state(|| Option::<RtcPeerConnection>::None);

    {
        // Clone the states once for use within the effect.
        let video_ref = video_ref.clone();
        let ws_state_inner = ws_state.clone();
        let pc_state_inner = pc_state.clone();

        use_effect_with_deps(move |_| {
            // 1. Set up local video stream.
            let window = window().expect("should have a window");
            let navigator = window.navigator();
            let media_devices = navigator.media_devices().expect("no media devices available");

            let constraints = MediaStreamConstraints::new();
            constraints.set_video(&JsValue::TRUE);
            let promise = media_devices
                .get_user_media_with_constraints(&constraints)
                .expect("getUserMedia should work");

            let video_ref_clone = video_ref.clone();
            let local_stream_future = async move {
                // Wait 3000 ms to give the DOM time to attach the video element.
                //gloo_timers::future::TimeoutFuture::new(3000).await;
            
                // Log the video element from the document directly.
                let doc = web_sys::window().unwrap().document().unwrap();
                let video_elem = doc.query_selector("video").unwrap();
                web_sys::console::log_1(&format!("document.querySelector('video'): {:?}", video_elem).into());
            
                // Now proceed to get the media stream.
                let stream = JsFuture::from(promise)
                    .await
                    .expect("failed to get media stream");
                let stream: MediaStream = stream.dyn_into().unwrap();
                web_sys::console::log_1(&"MediaStream obtained.".into());
            
                // Attempt to use the NodeRef to get the video element.
                if let Some(video) = video_ref_clone.cast::<HtmlVideoElement>() {
                    web_sys::console::log_1(&"Video element found via NodeRef. Setting srcObject.".into());
                    video.set_src_object(Some(&stream));
                    if let Err(err) = video.play() {
                        web_sys::console::log_1(&format!("Error playing video: {:?}", err).into());
                    } else {
                        web_sys::console::log_1(&"Video play() called successfully.".into());
                    }
                } else {
                    web_sys::console::log_1(&"No video element found via NodeRef even after delay.".into());
                }
                // Return the stream so it can be used later.
                stream
            };
            
            

            // Spawn an async block to set up the PeerConnection.
            {
                // Clone the state handles for use inside the async block.
                let pc_state_async = pc_state_inner.clone();
                let ws_state_async = ws_state_inner.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let local_stream = local_stream_future.await;

                    // 2. Initialize the RTCPeerConnection.
                    let config = RtcConfiguration::new();
                    let pc = RtcPeerConnection::new_with_configuration(&config)
                        .expect("Failed to create RTCPeerConnection");

                    // Add all tracks from the local stream to the PeerConnection.
                    let tracks = local_stream.get_tracks();
                    for i in 0..tracks.length() {
                        let track_value = tracks.get(i);
                        if track_value.is_undefined() {
                            continue;
                        }
                        // Convert the JsValue to a MediaStreamTrack.
                        let track: MediaStreamTrack = track_value
                            .dyn_into()
                            .expect("Failed to cast to MediaStreamTrack");
                        let empty_array = Array::new();
                        let _rtp_sender = pc.add_track(&track, &local_stream, &empty_array);
                    }

                    // 3. Set up the ICE candidate event handler.
                    {
                        let ws_state_for_ice = ws_state_async.clone();
                        let on_ice_candidate = Closure::wrap(Box::new(move |event: RtcPeerConnectionIceEvent| {
                            if let Some(candidate) = event.candidate() {
                                let candidate_str = candidate.candidate();
                                let msg = SignalMessage::IceCandidate(candidate_str);
                                let msg_json = serde_json::to_string(&msg).unwrap();
                                if let Some(ref ws) = *ws_state_for_ice {
                                    ws.send_with_str(&msg_json)
                                        .expect("Failed to send ICE candidate");
                                }
                            }
                        }) as Box<dyn FnMut(_)>);
                        pc.set_onicecandidate(Some(on_ice_candidate.as_ref().unchecked_ref()));
                        on_ice_candidate.forget();
                    }

                    // 4. Store the PeerConnection in state.
                    pc_state_async.set(Some(pc));
                });
            }

            // 5. Establish the WebSocket connection to the signaling server.
            {
                let ws_state_for_ws = ws_state_inner.clone();
                let ws_url = "ws://192.168.1.121:3030/ws"; // Replace with your actual local IP
                let ws = WebSocket::new(ws_url).expect("WebSocket creation failed");
                ws_state_for_ws.set(Some(ws.clone()));
            }

            // 6. Set up the WebSocket message handler.
            {
                let pc_state_for_message = pc_state_inner.clone();
                let ws_state_for_message = ws_state_inner.clone();
                let on_message = Closure::wrap(Box::new(move |event: MessageEvent| {
                    if let Some(text) = event.data().as_string() {
                        if let Ok(signal) = serde_json::from_str::<SignalMessage>(&text) {
                            if let Some(ref pc) = *pc_state_for_message {
                                match signal {
                                    SignalMessage::Offer(sdp) => {
                                        log::info!("Received offer: {}", sdp);
                                        // Additional offer handling here.
                                    },
                                    SignalMessage::Answer(sdp) => {
                                        log::info!("Received answer: {}", sdp);
                                        // Additional answer handling here.
                                    },
                                    SignalMessage::IceCandidate(candidate_str) => {
                                        let  candidate_init = RtcIceCandidateInit::new(&candidate_str);
                                        if let Ok(candidate) = web_sys::RtcIceCandidate::new(&candidate_init) {
                                            let _ = pc.add_ice_candidate_with_opt_rtc_ice_candidate(Some(&candidate));
                                            log::info!("Added ICE candidate: {}", candidate_str);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }) as Box<dyn FnMut(_)>);
                if let Some(ref ws) = *ws_state_for_message {
                    ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
                }
                on_message.forget();
            }

            || ()
        }, ());
    }

    html! {
        <div>
            <h1>{ "Rust Video Chat (Enhanced)" }</h1>
            <video ref={video_ref} autoplay=true playsinline=true muted=true
                   style="width: 480px; height: 360px; background: #000;" />
            <p>{ "This is the enhanced version with WebSocket signaling and WebRTC integration." }</p>
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn run_app() {
    yew::Renderer::<App>::new().render();
}
