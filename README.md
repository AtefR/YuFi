# YuFi

YuFi is a lightweight GTK4 Wi‑Fi manager for Linux, built in Rust. It focuses on a clean,
minimal dashboard for quick toggles, scanning, and network management.

## Status
Early scaffold: GTK4 UI layout only (no backend wiring yet).

## Features (planned)
- Enable/disable Wi‑Fi device
- Scan and list networks with quick connect/disconnect
- Connect to hidden networks
- View saved network details
- Edit IP/DNS configuration
- Reveal saved password
- Manage auto‑reconnect per network

## Roadmap
- Phase 1: UI scaffold + theming (in progress)
- Phase 2: NetworkManager D‑Bus backend wiring
- Phase 3: Connect flows, saved network details, hidden networks
- Phase 4: IP/DNS editor, password reveal, auto‑reconnect
- Phase 5: Packaging and releases (Flatpak/AppImage)

## Build
Requires GTK4 development libraries.

```
cargo run
```
