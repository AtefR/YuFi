# YuFi

YuFi is a lightweight GTK4 Wi‑Fi manager for Linux, built in Rust. It focuses on a clean,
minimal dashboard for quick toggles, scanning, and network management.

## Status
Stable 1.0.0 release. Ongoing maintenance and UX polish.

## Install
### Arch (AUR)
```
yay -S yufi
```

Binary release:
```
yay -S yufi-bin
```

### GitHub Releases (manual)
Download the release tarball and extract it to `/` (it contains a `usr/` tree).
```
tar -xzf yufi-1.0.0-x86_64.tar.gz -C /
```

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
cargo build --release
./target/release/yufi
```

## Permissions
YuFi uses NetworkManager via D‑Bus. Changing network settings or revealing saved passwords
requires a running polkit agent (for example `polkit-gnome`, `lxqt-policykit`, or `mate-polkit`).

Optional: a sample polkit rule is provided at `packaging/polkit/90-yufi.rules` if you want to
allow active local users to manage Wi‑Fi without repeated prompts. Review it before installing.

Flatpak builds already request system‑bus access in `packaging/com.yufi.app.yml`, but permissions
still depend on host polkit policy.

## Packaging
- AUR source: `packaging/aur/yufi`
- AUR binary: `packaging/aur/yufi-bin`
- Release script: `packaging/release.sh`
- Desktop entry: `packaging/com.yufi.app.desktop`
- Icon: `packaging/com.yufi.app.svg`
- Flatpak: `packaging/com.yufi.app.yml`
- AppImage: optional for portable builds

## Notes
- Password reveal requires appropriate permissions (polkit/NetworkManager).
- UI operations run off the main thread to keep the app responsive.
