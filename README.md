# YuFi

YuFi is a lightweight GTK4 Wi‑Fi manager for Linux, built in Rust. It focuses on a clean,
minimal dashboard for quick toggles, scanning, and network management.

## Status
Active development. UI and core NetworkManager integration are in place; some advanced features
are still being refined.

## Features
- Enable/disable Wi‑Fi device
- Scan and list networks with quick connect/disconnect
- Connect to hidden networks
- View saved network details
- Edit IP/DNS configuration (IP, prefix, gateway, DNS)
- Reveal saved password (if permissions allow)
- Manage auto‑reconnect per network

## Build
Requires GTK4 development libraries and NetworkManager.

```
cargo build
cargo run
```

## Packaging (draft)
- Desktop entry: `com.yufi.app.desktop`
- Icon: `com.yufi.app.svg`
- Flatpak: recommended for distribution
- AppImage: optional for portable builds

## Notes
- Password reveal requires appropriate permissions (polkit/NetworkManager).
- UI operations run off the main thread to keep the app responsive.
