# BetterFT8

A web-based FT8 ham radio client. The server runs on any machine connected to your radio — Linux, Windows, macOS, or a Raspberry Pi. You operate from any browser, no software installation required.

**Features**
- Live waterfall display with decoded message overlays
- Automated QSO state machine (CQ → exchange → 73 → ADIF log)
- CAT radio control via [Hamlib](https://hamlib.org/) `rigctld`
- Multi-client viewing — one operator at a time, unlimited viewers
- TLS support for remote operation over the internet
- Settings UI (callsign, grid, audio devices, radio config)

## Architecture

```
Radio ──audio──► Rust server ──WebSocket──► Browser (Svelte SPA)
Radio ◄──CAT──── rigctld ◄──── server
```

The server (`server/`, Rust + Axum) handles all hardware: audio capture, FT8 decode/encode via the [ft8_lib](https://github.com/kgoba/ft8_lib) C library, PTT, and CAT control. The client (`client/`, Svelte 5) is a single-page app served from the same port — open `http://<server>:8073` in a browser.

## Requirements

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+
- [Hamlib](https://github.com/Hamlib/Hamlib/releases) `rigctld` (for radio control)
- A C compiler (MSVC on Windows, gcc/clang on Linux/macOS)

## Dev setup

cd client && npm run dev
cd server && cargo watch -x "run --"



## Quick Start

**1. Build the client**
```bash
cd client
npm install
npm run build
```

**2. Configure**

Copy and edit `betterft8.toml`:
```toml
[network]
host = "0.0.0.0"
port = 8073
static_files = "../client/dist"
operator_password = "changeme"

[station]
callsign = "W1AW"
grid = "FN31"

[audio]
# input_device = "USB Audio CODEC"   # omit for system default
# output_device = "USB Audio CODEC"  # omit for system default
sample_rate = 48000

[radio]
rigctld_host = "localhost"
rigctld_port = 4532
poll_interval_ms = 2000
```

**3. Start rigctld** (in a separate terminal)
```bash
# Example: Icom IC-7300 on COM3 (Windows) or /dev/ttyUSB0 (Linux)
rigctld -m 3073 -r COM3 -s 19200
```

For testing without a radio, use the dummy rig:
```bash
rigctld -m 1 -r NUL        # Windows
rigctld -m 1 -r /dev/null  # Linux/macOS
```

**4. Run the server**
```bash
cd server
cargo run --release -- ../betterft8.toml
```

Open `http://localhost:8073` in a browser.

## Radio Setup

See [docs/testing-with-radio.md](docs/testing-with-radio.md) for detailed instructions including:
- Finding your radio's Hamlib model number
- CAT configuration for Icom, Yaesu, and Kenwood radios
- Audio device setup (USB audio, SignaLink, line-in)
- FT8 frequencies by band

Common Hamlib model numbers:

| Radio | Model |
|-------|-------|
| Icom IC-7300 | 3073 |
| Icom IC-7610 | 3078 |
| Icom IC-705 | 3087 |
| Yaesu FT-991A | 1035 |
| Yaesu FT-DX10 | 1108 |
| Kenwood TS-590SG | 228 |
| Kenwood TS-890S | 243 |
| Elecraft K3 | 2029 |

## Multi-User / Remote Operation

Any number of clients can connect and view the waterfall and decoded messages. Only one client at a time can claim the operator role (required for TX and radio control).

- **`operator_password`** — required to claim the operator role
- **`viewer_password`** — if set, all clients must authenticate before seeing anything

For remote operation over the internet, enable TLS in `betterft8.toml`:
```toml
[network]
tls_cert = "/path/to/cert.pem"
tls_key  = "/path/to/key.pem"
```
Then connect via `https://<your-server>:8073`.

## Linux / Raspberry Pi Deployment

A systemd service unit is included at `deploy/betterft8.service`. Copy the compiled binary and client dist to `/home/pi/betterft8/`, then:

```bash
sudo cp deploy/betterft8.service /etc/systemd/system/
sudo systemctl enable --now betterft8
```

Build a release binary for ARM:
```bash
cross build --release --target aarch64-unknown-linux-gnu
```

## License

GPL-3.0 — inherited from [ft8_lib](https://github.com/kgoba/ft8_lib) by Karlis Goba.
