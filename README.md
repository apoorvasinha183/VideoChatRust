# VideoChatRust

A Rust-based WebRTC video chat application. This repository contains two primary projects:

- **frontend**: The WebAssembly (WASM)–based client built with [Yew](https://yew.rs/) and bundled with [Trunk](https://trunkrs.dev/).
- **signaling**: A signaling server implemented in Rust that uses WebSockets to exchange SDP offers/answers and ICE candidates for establishing WebRTC connections.

The application currently supports one-to-one video chat. Future plans include adding multi-peer conferencing and additional media controls (e.g., volume, mute).

## Repository Structure

```plaintext
.
├
├── Cargo.toml           # Workspace (if applicable) or root config
├── frontend             # Frontend (client) project
│   ├── Cargo.toml
│   ├── Trunk.toml       # Trunk configuration for building the WASM app
│   ├── dist             # Distribution folder after build
│   │   ├── frontend-945f5068871ac5a0.js
│   │   ├── frontend-945f5068871ac5a0_bg.wasm
│   │   └── index.html
│   ├── index.html       # Entry point for the app
│   ├── src
│   │   ├── lib.rs       # Main Yew application code
│   │   └── visitor_counter.rs  # Visitor counter component/module
│   └── visitor-counter.js  # Additional JavaScript interop code (if needed)
├── ngrok.yml            # Ngrok configuration for exposing the app (if needed)
└── signaling            # Signaling server project
    ├── Cargo.toml
    └── src
        └── main.rs      # Entry point for the signaling server
```

## Prerequisites

- **Rust:** Install via [rustup](https://rustup.rs/). A recent stable version is recommended.
- **Trunk:** A WASM bundler for Rust projects. Install with:
  ```bash
  cargo install trunk
  ```
- **Ngrok (optional):** For exposing your application externally (e.g., for mobile testing).

## Building and Running

### Frontend

1. **Development:**

   - Navigate to the `frontend` directory:
     ```bash
     cd frontend
     ```
   - Build and serve the application using Trunk:
     ```bash
     trunk serve
     ```
   - By default, Trunk serves the app at [http://127.0.0.1:8080](http://127.0.0.1:8080).

2. **Production Build:**

   - To create a production build, run:
     ```bash
     trunk build --release
     ```
   - The compiled files (including WASM and JS) will be output to the `dist` directory.

### Signaling Server

1. **Build and Run:**

   - Navigate to the `signaling` directory:
     ```bash
     cd signaling
     ```
   - Run the signaling server:
     ```bash
     cargo run
     ```
   - The server is configured (in your code) to listen on a specified port (e.g., `3030`). Ensure this port is correctly routed if using a reverse proxy.

### Ngrok (Optional)

If you need to expose your application over the Internet (for instance, for testing on a mobile device):

1. Ensure your `ngrok.yml` is properly configured.
2. Start Ngrok:
   ```bash
   ngrok start --config=ngrok.yml
   ```
3. Ngrok will forward external traffic to your locally running frontend/signaling server.You need the https address for the video chat to run in a browser.

## Features and Future Improvements

- **Current Features:**
  - One-to-one video chat using WebRTC.
  - A WASM-based frontend built with Yew.
  - A Rust-based signaling server for SDP and ICE candidate exchange.

- **Planned Enhancements:**
  - **Multi-Peer Conferencing:** Expand the architecture to support video conferences with multiple participants.
  - **Additional Controls:** Add UI elements for volume control, mute/unmute, and other media settings.
  - **Code Refactoring:** Improve modularity by separating signaling logic, peer connection management, and UI components.
  - **Enhanced Error Handling:** Replace `expect()` calls with robust error handling and structured logging.

## Contributing

Contributions are welcome! If you have suggestions, improvements, or fixes, please fork the repository and submit a pull request or open an issue.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.


```
