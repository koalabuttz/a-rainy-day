# a rainy day

A playable first check-in at the forest inn. Ask for food or a room, listen near the far end of the counter, and see how the innkeeper reacts when you bring up what you heard.

## Play in a browser

Install stable Rust with rustup and have Python 3 available. From the project root, run:

```sh
rustup target add --toolchain stable wasm32-unknown-unknown
python3 tools/build_web.py
python3 -m http.server 8000 --bind 127.0.0.1 --directory target/web
```

Open [the room on localhost](http://127.0.0.1:8000/). Use WASD or the arrow keys and press E to talk or listen. On touch screens, use the direction buttons and tap Talk or Listen when it appears. Ask in your own words or select a reply. The generated `target/web/index.html` embeds the WASM and art, so the browser build is a single file.

## Build for PS Vita

Install VitaSDK and its SDL2 package. Set `VITASDK` to the SDK location and put its `bin` directory on `PATH`. From the project root, run:

```sh
rustup toolchain install nightly --profile minimal --component rust-src
cargo +nightly install --locked cargo-vita --version 0.2.2
cargo +nightly vita build vpk --release --package rainy-day-vita --locked
```

The package is `target/armv7-sony-vita-newlibeabihf/release/rainy-day-vita.vpk`. Use the D-pad to move or highlight a reply, Cross to select, Circle to step away, and Start to exit. Cross-compilation and archive validation passed with Rust nightly 2026-11-18, VitaSDK snapshot `20261015.850.1`, and SDL2 2.32.8. Launch and controller checks on Vita hardware or an emulator are pending.

## Play on Nintendo DS

Open `target/ds/a-rainy-day.nds` in a DS emulator or use a homebrew loader on a DS. Use the D-pad to walk or highlight a reply, A to talk, listen, or select, and B to step away. The top screen shows the room at native pixel size and pans left or right as you move. The bottom screen stays black.

DeSmuME checks covered boot, all four directions, D-pad release, camera panning, and wall and counter collision. Physical hardware testing is pending.

To build the ROM, install Rust nightly with `rust-src` and [BlocksDS](https://blocksds.skylyrac.net/docs/setup/). Run these commands from the project root.

```sh
rustup toolchain install nightly --profile minimal --component rust-src
CARGO_TARGET_ARMV5TE_NONE_EABI_RUSTFLAGS='-C target-cpu=arm946e-s' \
  cargo +nightly build --locked -Z build-std=core,alloc \
  --target armv5te-none-eabi --profile nds -p rainy-day-ds
make -C platform/ds
```

Set `BLOCKSDS` and `WONDERFUL_TOOLCHAIN` to your SDK paths if they differ from the defaults in the Makefile. With the [BlocksDS Docker image](https://blocksds.skylyrac.net/docs/setup/docker/), run the Rust command on the host and run `make` inside the container.

### Use the DS microphone

Speech is optional and runs through a companion on a computer that shares the DS's local network. Set up a saved connection in the DS's Nintendo Wi-Fi settings first. Most current Wi-Fi networks require a security mode the original DS cannot join, so use a trusted, isolated DS-compatible network for testing.

1. On the companion computer, create a Python environment and install Faster Whisper. Replace the example IP with the computer's address on that network.

   ```sh
   python3 -m venv ~/.venvs/a-rainy-day-voice
   ~/.venvs/a-rainy-day-voice/bin/python -m pip install faster-whisper==1.2.1
   ~/.venvs/a-rainy-day-voice/bin/python tools/voice_companion.py --bind 192.168.4.2
   ```

2. Put a file named `a-rainy-day-voice.txt` at the root of the flashcart or loader's SD card. Put only the companion's IPv4 address on its first line. Allow the DS to reach TCP port 8765 on the companion.
3. Talk to the innkeeper. Hold X, wait for **Speak now**, speak for up to five seconds, and release X. Check the transcript on the top screen. Press A to use it or B to discard it.

The first companion launch downloads an English Whisper model. Audio travels over the local network and is transcribed on the companion in memory. The shared dialogue recognizes questions about the room, food, and staff from both spoken transcripts and browser text. Button choices work offline.

## Check movement and review the art

- `cargo +stable test --workspace --locked` exercises the route from the entrance to the counter and collision with the wall, innkeeper, and frog.
- Desktop and phone-size Chromium checks covered keyboard and touch movement, stopping at the counter, release on blur, touch cancellation, and full-room framing.
- The browser, Vita, and DS launchers share the same 320×176 Rust renderer and 16 px tile grid. PixelLab's native 16×16 frog retains its 14×13 px silhouette and readable eyes at 1×. The PixelLab innkeeper fits a 24 px silhouette, with the reception counter in front. The traveler has four standing views and six-frame walk loops in each direction, drawn about 22 px tall.
- The RetroDiffusion floor tile repeats across both axes, and the back-wall tile repeats horizontally under the existing posts and windows. The renderer dims both textures to keep the characters readable. The same grain marks remain visible at desktop scale. Add a matching tile variant if that pattern becomes distracting in larger rooms.
- [Asset sources](assets/sources.json) records the selected art manifests for the traveler and room fixtures. Static PNGs are copied as returned. The renderer crops the window around the sash, trims sprite padding, and scales with nearest-neighbor sampling. The traveler atlas includes six retouched south-facing poses. To rebuild it with Pillow installed, run `python3 -B tools/pack_traveler.py ROTATION_MANIFEST WALK_MANIFEST` using the walk manifest and its `source_manifest`.
- Generate art at the intended size with `python3 -B tools/graphics_trial.py generate pixellab room-frog` or `python3 -B tools/graphics_trial.py generate retrodiffusion room-floor`. Add `--seed 658` to change only the seed. Review specific manifests with `python3 -B tools/review_art.py path/to/manifest.json`, then judge the art in the room at 1×. Local trials live in `art/experiments/`. The repo tracks selected frames and generation settings in `art/sources/`.
