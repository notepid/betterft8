# Hamlib Library

This directory contains the Hamlib library files needed for the direct `hamlib` radio backend.

## Windows (automatic)

`hamlib-w64.zip` is included in the repository. The build system extracts it automatically on first build — no manual steps needed. The zip contains:

- `hamlib.lib` — MSVC import library (link-time)
- `libhamlib-4.dll` — Hamlib runtime library
- `libgcc_s_seh-1.dll`, `libusb-1.0.dll`, `libwinpthread-1.dll` — runtime dependencies

The DLLs must be next to the server executable at runtime. After building, copy them from this directory to the `target/debug/` or `target/release/` directory, or add this directory to your PATH.

### Updating the zip

To update to a newer Hamlib version:

1. Download the latest `hamlib-w64-*.zip` from https://github.com/Hamlib/Hamlib/releases
2. Extract it somewhere temporary
3. Generate the MSVC import library from the `.def` file:
   ```
   lib /def:lib\msvc\libhamlib-4.def /out:hamlib.lib /machine:x64
   ```
4. Create a new zip containing: `hamlib.lib`, `hamlib.exp`, `libhamlib-4.dll`, `libgcc_s_seh-1.dll`, `libusb-1.0.dll`, `libwinpthread-1.dll`
5. Replace `hamlib-w64.zip` in this directory

## Linux

Install the system package and the build system will find it:

```bash
# Debian/Ubuntu
sudo apt install libhamlib-dev

# Fedora
sudo dnf install hamlib-devel
```

## Building without Hamlib

To build with only the rigctld TCP backend (no Hamlib dependency):

```bash
cargo build --no-default-features
```
