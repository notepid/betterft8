# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Project Is

BetterFT8 is a web-based FT8 ham radio client/server application. The Rust server captures audio from a radio, decodes FT8 signals, manages a QSO state machine, and controls a transceiver via Hamlib's `rigctld`. A Svelte 5 SPA in the browser provides real-time waterfall display, decoded message list, and radio control.

## Commands

### Server (Rust)
```bash
cd server
cargo build                  # debug build
cargo build --release        # release build
cargo run -- ../betterft8.toml   # run with config
RUST_LOG=debug cargo run -- ../betterft8.toml  # verbose logging
```

### Client (Svelte/Vite)
```bash
cd client
npm install
npm run dev     # dev server on localhost:5173
npm run build   # compile to client/dist/
npm run preview # preview production build
```

### Running Together
1. `cd client && npm run build` — produces static files in `client/dist/`
2. Set `static_files = "client/dist"` in `betterft8.toml`
3. `cd server && cargo run -- ../betterft8.toml` — serves everything on port 8073

For UI-only development without a radio, use Hamlib's dummy rig:
```bash
rigctld -m 1   # dummy rig on localhost:4532
```
See `docs/testing-with-radio.md` for hardware setup.

## Architecture

### Data Flow
```
cpal audio input → 12kHz ring buffer → FFT → waterfall broadcast (100ms)
                                     → ft8_decode_audio() [C lib] → decodes broadcast (15s)
Client TX request → QSO state machine → ft8_encode_audio() [C lib] → cpal playback + PTT
Browser ↔ WebSocket (/ws) ← → Axum server (port 8073)
rigctld daemon ← → hamlib.rs TCP client (polls every 2s)
```

### Server (`server/src/`)
- **`main.rs`** — Loads config, spawns background tasks (waterfall DSP, FT8 timing engine, radio polling), starts Axum.
- **`state.rs`** — `AppState` with shared channels, mutexes, and broadcast payload types.
- **`audio/`** — `capture.rs` (cpal → decimated 12kHz ring buffer), `playback.rs` (TX audio output).
- **`dsp/`** — `fft.rs` (4096-pt realfft), `ft8.rs` (C wrapper calls), `waterfall.rs` (FFT → base64 → broadcast).
- **`engine/`** — `timing.rs` (UTC-synced 15s decode loop, TX window), `qso.rs` (QSO state machine), `logger.rs` (ADIF output).
- **`radio/`** — `hamlib.rs` (async TCP rigctld client with extended `+\cmd` protocol).
- **`web/`** — `server.rs` (router), `ws_handler.rs` (select! loop), `session.rs` (per-client mpsc channels, operator lock), `messages.rs` (serde message types).

### Client (`client/src/`)
- **`lib/websocket.ts`** — `BetterFT8Client` class; reconnect logic, dispatches incoming messages to stores.
- **`lib/stores.ts`** — Svelte stores: `connected`, `myCall`, `radioStatus`, `waterfallLine`, `decodes`, etc.
- **`components/Waterfall.svelte`** — Canvas waterfall with decode overlays and click-to-select.
- Other components map 1:1 to UI panels: `DecodeList`, `RadioStatus`, `Controls`, `QsoPanel`, `Login`, `Settings`.

### FT8 C Library
- Vendored at `server/ft8_lib/` (Karlis Goba, GPL-3.0).
- Compiled by `server/build.rs` using the `cc` crate.
- Wrapper: `server/ft8_wrapper.{h,c}` exposes `ft8_decode_audio()` and `ft8_encode_audio()`.
- MSVC patches: VLA array limits in `decode.c` and `monitor.c`, `msvc_compat.h` for `stpcpy`.

## Configuration (`betterft8.toml`)
```toml
[network]
host = "0.0.0.0"
port = 8073
static_files = "client/dist"
operator_password = "secret"   # required to TX/control radio
viewer_password = ""           # optional; leave empty for open viewing

[station]
callsign = "W1AW"
grid = "FN31"

[audio]
input_device = ""    # empty = system default
sample_rate = 48000

[radio]
rigctld_host = "localhost"
rigctld_port = 4532
poll_interval_ms = 2000
```

Config is loaded at startup and live-editable via the Settings panel (saved back to disk).

## Key Design Decisions
- WebSocket messages are JSON with a `type` discriminant (no binary protocol).
- One operator lock at a time; all other clients are read-only viewers.
- Audio is always decimated to 12kHz mono internally (FT8 bandwidth requirement).
- FT8 timing is hard-synced to UTC (decode at 13s into each 15s window).
- GPL-3.0 license inherited from ft8_lib.
