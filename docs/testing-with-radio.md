# Testing BetterFT8 with a Real Radio (Windows)

## Prerequisites

- A radio supported by Hamlib (most modern transceivers are)
- A CAT/CI-V cable connecting your radio to the PC (USB-serial or native USB)
- An audio interface: either the radio's built-in USB audio, a SignaLink, or a sound card connected to the radio's accessory port

---

## 1. Install Hamlib

1. Go to the [Hamlib releases page](https://github.com/Hamlib/Hamlib/releases)
2. Download the latest `hamlib-w64-x.x.x.zip` (64-bit Windows build)
3. Extract to a permanent location, e.g. `C:\hamlib\`
4. Optionally add `C:\hamlib\bin` to your system PATH so you can run `rigctld` from any terminal

---

## 2. Find Your Rig Model Number

Look up your radio in the Hamlib rig list:

```
C:\hamlib\bin\rigctl.exe -l
```

This prints all supported rigs with their model numbers. Common ones:

| Radio              | Model |
|--------------------|-------|
| Icom IC-7300       | 3073  |
| Icom IC-7610       | 3078  |
| Icom IC-705        | 3087  |
| Yaesu FT-991A      | 1035  |
| Yaesu FT-DX10      | 1108  |
| Kenwood TS-590SG   | 228   |
| Kenwood TS-890S    | 243   |
| Elecraft K3        | 2029  |
| Elecraft KX3       | 2045  |

---

## 3. Find Your Serial Port

1. Connect the CAT cable and power on the radio
2. Open **Device Manager** → expand **Ports (COM & LPT)**
3. Note the COM port, e.g. `COM3`

For Icom radios using CI-V over USB, the radio usually presents as two COM ports — use the lower-numbered one.

---

## 4. Configure Your Radio for CAT

Each radio has different menu settings. Common things to check:

**Icom (CI-V):**
- Set CI-V baud rate in the radio menu (e.g. 19200 or 115200)
- Enable CI-V transceive if you want unsolicited updates

**Yaesu:**
- Enable CAT in the menu, set baud rate (typically 38400 for FT-991A)
- Set CAT RTS to "Enable" if needed

**Kenwood:**
- Set COM speed in the menu (typically 9600 or 57600)

---

## 5. Start rigctld

Open a terminal and run:

```
C:\hamlib\bin\rigctld.exe -m <model> -r COM3 -s <baudrate>
```

Examples:

```
# Icom IC-7300 on COM3 at 19200 baud
C:\hamlib\bin\rigctld.exe -m 3073 -r COM3 -s 19200

# Yaesu FT-991A on COM5 at 38400 baud
C:\hamlib\bin\rigctld.exe -m 1035 -r COM5 -s 38400

# Kenwood TS-590SG on COM4 at 57600 baud
C:\hamlib\bin\rigctld.exe -m 228 -r COM4 -s 57600
```

Leave this terminal open — rigctld must stay running while BetterFT8 is active.

**Verify it works** (in a second terminal):

```
C:\hamlib\bin\rigctl.exe -m 2 f
```

This should print the current VFO frequency. If it does, BetterFT8 will be able to read and control the radio.

---

## 6. Configure Audio

BetterFT8 needs to receive audio from the radio. Options:

**USB audio (Icom IC-7300, IC-705, etc.):**
- The radio appears as a USB audio device in Windows
- Set it as the default recording device, or configure `betterft8.toml`:
  ```toml
  [audio]
  input_device = "USB Audio CODEC"   # exact name from your system
  sample_rate = 48000
  ```

**SignaLink USB:**
- Appears as a USB audio device; set input to the SignaLink device name

**Built-in sound card via ACC port:**
- Connect radio audio out → PC line in
- Set input to the appropriate line-in device

To see available device names, run the server once and check the startup log — it lists available audio devices.

---

## 7. Set the Radio to an FT8 Frequency

Put the radio in USB-D (or DATA) mode and tune to an FT8 frequency:

| Band | Frequency  |
|------|------------|
| 160m | 1.840 MHz  |
| 80m  | 3.573 MHz  |
| 40m  | 7.074 MHz  |
| 30m  | 10.136 MHz |
| 20m  | 14.074 MHz |
| 17m  | 18.100 MHz |
| 15m  | 21.074 MHz |
| 12m  | 24.915 MHz |
| 10m  | 28.074 MHz |
| 6m   | 50.313 MHz |

You can also use the band buttons in the BetterFT8 browser UI to tune the radio directly.

---

## 8. Update betterft8.toml

```toml
[station]
callsign = "YOUR_CALL"
grid = "XX00"          # your 4-character Maidenhead grid square

[audio]
# input_device = "USB Audio CODEC"   # uncomment and set if not using default
sample_rate = 48000    # match your audio device; server will decimate to 12kHz

[radio]
rigctld_host = "localhost"
rigctld_port = 4532
poll_interval_ms = 2000
```

---

## 9. Build and Run

```
# Terminal 1 — rigctld (keep running)
C:\hamlib\bin\rigctld.exe -m 3073 -r COM3 -s 19200

# Terminal 2 — BetterFT8 server
cd C:\...\betterft8\server
cargo run --release

# Build client (first time or after changes)
cd C:\...\betterft8\client
npm run build
```

Open a browser to `http://localhost:8073`

---

## Troubleshooting

**"rigctld not available" in server log**
- Check that rigctld is running (`rigctl.exe -m 2 f` should return a frequency)
- Check that port 4532 is not blocked by a firewall
- Verify `rigctld_port` in `betterft8.toml` matches rigctld's listen port (default 4532)

**No audio / waterfall is blank**
- Check the audio device name in `betterft8.toml`
- Make sure the radio audio output level is reasonable (not clipping, not silent)
- On Icom USB radios, check that the MOD source is set to USB in the radio menu

**Frequency display shows 0 Hz**
- The radio may not have responded before the first poll; wait a few seconds
- Try a lower CI-V/CAT baud rate

**rigctld can't open the COM port**
- Make sure no other software (WSJT-X, fldigi, etc.) has the port open
- Try running the terminal as Administrator
- Check the exact COM port number in Device Manager

---

## Testing Without a Radio (Dummy Rig)

To test the UI and server without any hardware:

```
C:\hamlib\bin\rigctld.exe -m 1 -r NUL
```

Model 1 is Hamlib's built-in dummy rig — it responds to all commands with simulated values. The frequency will show as 14.200 MHz and you can use the band buttons to change it.
