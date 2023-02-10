# ChatImproVR
Crates:
* `client`: Client application, provides rendering, input, and other user interfacing
* `server`: Server application, a headless service
* `engine`: WASM Plugin, ECS, and messaging layer for use in implementing server and client
* `engine_interface`: Engine interface for use within e.g. plugins
* `common`: Interfacing data types between provided plugin, client, and server e.g. position component
* `plugin`: An example plugin (currently moves the camera)
* `plugin2`: An example plugin (currently adds and moves cubes)

Plugins are required to import the `engine_interface` crate, and often import the `common` crate

# Preparation
Make sure you have the `wasm32-unknown-unknown` target installed;
```sh
rustup target add wasm32-unknown-unknown
```

Dependencies on Ubuntu:
```sh
sudo apt install build-essential cmake libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libspeechd-dev libxkbcommon-dev libssl-dev
```

# Compilation and running
First, compile the plugins. Currently you must `cd` into each plugin and build it with `cargo build --release`.

Next, execute:
```sh
cd server && \
cargo run --release --bin cimvr_server -- ../target/wasm32-unknown-unknown/release/plugin*.wasm
```

Finally, in **another terminal**, execute:
```sh
cd client && \
cargo run --release --bin cimvr_client -- ../target/wasm32-unknown-unknown/release/plugin*.wasm
```

# Connection to a remote server
You may connect to a remote server like so:
```sh
cargo run --release --bin cimvr_client -- -connect <ip>:5031 <plugins>
```

The default port is 5031, but this can be configured in the server with `--bind <bind addr>:<port>`

# Organization 
![Visual aid for crate graph](./graph.svg)

Plugins are required to import `engine_interface`. Most plugins will need to import `common`, as it provides interfacing with the provided client and server. The `engine` and `engine_interface` crates are all that are needed to set up arbitrary new platforms...

# TODO
* [x] Use the `log` crate for errors and warnings host-size
* [x] Interface for server-client messaging
* [ ] Display plugin names along with print. Should happen in log...
* [ ] Use real UUIDs instead of these random numbers and silly ID constants
* [ ] All of the other TODOs... `grep -ir 'todo' */src/*`
* [ ] Loading bar for plugins
* [x] Networking!
* [ ] Optionally-unreliable networking (faster but tradeoff packet loss, streamed instead of diff'd) 
* [x] Figure out how to organize code for serverside/clientside easier. Should be able to compile for both...
* [x] Hot-reloading
* [x] Trigger hot reload on wasm file change
* [ ] Limits on plugin resources; execution time, message and component sizes, etc
* [ ] VR support (OpenXR)
* [ ] VR support for GUI (OpenXR keyboard?)

Access components:
* `Synchronized`: Object is sent from server to client periodically
* `Saved`: Entity and associated components written to disk 
    * Different owners? Like clients' loaded plugins should be able to retrieve data on exit
    * 'Guest' plugins alongside client and server...
