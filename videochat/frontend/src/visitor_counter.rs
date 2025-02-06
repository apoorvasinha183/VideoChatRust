use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::window;
use yew::prelude::*;

#[function_component(VisitorCounter)]
pub fn visitor_counter() -> Html {
    // use_effect to inject the visitor counter script once when the component mounts
    use_effect(|| {
        if let Some(document) = window().and_then(|w| w.document()) {
            // Create a new <script> element.
            if let Ok(script) = document.create_element("script") {
                script.set_text_content(Some(
r#"(function () {
    const wsUrl = `wss://${window.location.host}/visitors`;
    const countElem = document.getElementById("visitor-count");
    if (!countElem) {
      console.error("No element with id 'visitor-count' found.");
      return;
    }
    const ws = new WebSocket(wsUrl);
    ws.onopen = function () {
      console.log("Connected to visitor counter.");
    };
    ws.onmessage = function (event) {
      try {
        const data = JSON.parse(event.data);
        if (data.visitorCount !== undefined) {
          countElem.textContent = data.visitorCount;
        }
      } catch (err) {
        console.error("Error parsing visitor count message:", err);
      }
    };
    ws.onclose = function () {
      console.log("Disconnected from visitor counter.");
    };
    window.addEventListener("beforeunload", function () {
      ws.close();
    });
})();"#
                ));
                // Append the script to the body.
                if let Some(body) = document.body() {
                    let _ = body.append_child(&script);
                }
            }
        }
        || ()
    });

    html! {
        html! {
            <div id="visitor-counter" style="margin-top: 20px; font-size: 18px; color: #000;">
                { "This site has currently " }
                <span id="visitor-count" style="color: #000;">{ "0" }</span>
                { " visitors" }
            </div>
        }
    }
}
