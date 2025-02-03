use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlVideoElement, MediaStream, MediaStreamConstraints, window};
use yew::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[function_component(App)]
fn app() -> Html {
    // A reference to the <video> element.
    let video_ref = NodeRef::default();

    {
        let video_ref = video_ref.clone();
        use_effect_with_deps(move |_| {
            // Get the global window and navigator objects.
            let window = window().expect("should have a window");
            let navigator = window.navigator();
            let media_devices = navigator.media_devices().expect("no media devices available");

            // Set constraints: video enabled (audio optional).
            let mut constraints = MediaStreamConstraints::new();
            constraints.video(&JsValue::TRUE);
            // constraints.audio(&JsValue::TRUE); // Uncomment if you want audio

            // Request access to the media devices.
            let promise = media_devices.get_user_media_with_constraints(&constraints)
                .expect("getUserMedia should work");
            let future = async move {
                // Wait for the promise to resolve.
                let stream = JsFuture::from(promise).await
                    .expect("failed to get media stream");
                let stream: MediaStream = stream.dyn_into().unwrap();
                // Set the stream as the source for the video element.
                if let Some(video) = video_ref.cast::<HtmlVideoElement>() {
                    video.set_src_object(Some(&stream));
                    let _ = video.play();
                }
            };
            wasm_bindgen_futures::spawn_local(future);
            || ()
        }, ());
    }

    html! {
        <div>
            <h1>{ "Rust Video Chat (Proof of Concept)" }</h1>
            <video ref={video_ref} autoplay=true playsinline=true
                   style="width: 480px; height: 360px; background: #000;" />
            // In a full implementation, you’d add:
            // - A second <video> element for the remote stream.
            // - Buttons to start/stop the call.
            // - WebSocket handling to connect to the signaling server.
            // - WebRTC peer connection setup.
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn run_app() {
    yew::Renderer::<App>::new().render();
}
