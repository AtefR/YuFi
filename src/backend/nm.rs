use crate::backend::{Backend, BackendError, BackendResult};
use crate::models::{AppState, Network, NetworkAction};
use std::collections::HashMap;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedObjectPath;

pub struct NetworkManagerBackend;

impl NetworkManagerBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Backend for NetworkManagerBackend {
    fn load_state(&self) -> BackendResult<AppState> {
        let conn = system_bus()?;
        let nm = nm_proxy(&conn)?;

        let wifi_enabled: bool = nm
            .get_property("WirelessEnabled")
            .map_err(|e| BackendError::Unavailable(e.to_string()))?;

        let wifi_device = first_wifi_device(&conn, &nm)?;
        let wireless = wireless_proxy(&conn, &wifi_device)?;

        let active_ap: OwnedObjectPath = wireless
            .get_property("ActiveAccessPoint")
            .map_err(|e| BackendError::Unavailable(e.to_string()))?;

        let ap_paths: Vec<OwnedObjectPath> = wireless
            .call("GetAccessPoints", &())
            .map_err(|e| BackendError::Unavailable(e.to_string()))?;

        let mut best_by_ssid: HashMap<String, (u8, bool, &'static str)> = HashMap::new();

        for ap_path in ap_paths {
            let ap_proxy = ap_proxy(&conn, &ap_path)?;
            let ssid_bytes: Vec<u8> = ap_proxy
                .get_property("Ssid")
                .map_err(|e| BackendError::Unavailable(e.to_string()))?;
            let ssid = String::from_utf8_lossy(&ssid_bytes).trim().to_string();
            if ssid.is_empty() {
                continue;
            }

            let strength: u8 = ap_proxy
                .get_property("Strength")
                .map_err(|e| BackendError::Unavailable(e.to_string()))?;

            let is_active = ap_path == active_ap;
            let icon = icon_for_strength(strength);

            match best_by_ssid.get(&ssid) {
                Some((best_strength, best_active, _)) => {
                    if is_active && !best_active || strength > *best_strength {
                        best_by_ssid.insert(ssid, (strength, is_active, icon));
                    }
                }
                None => {
                    best_by_ssid.insert(ssid, (strength, is_active, icon));
                }
            }
        }

        let mut networks: Vec<Network> = best_by_ssid
            .into_iter()
            .map(|(ssid, (strength, is_active, icon))| Network {
                ssid,
                signal_icon: icon,
                action: if !wifi_enabled {
                    NetworkAction::None
                } else if is_active {
                    NetworkAction::Disconnect
                } else {
                    NetworkAction::Connect
                },
                strength,
                is_active,
            })
            .collect();

        networks.sort_by(|a, b| {
            b.is_active
                .cmp(&a.is_active)
                .then_with(|| b.strength.cmp(&a.strength))
                .then_with(|| a.ssid.cmp(&b.ssid))
        });

        Ok(AppState {
            wifi_enabled,
            networks,
        })
    }

    fn set_wifi_enabled(&self, _enabled: bool) -> BackendResult<()> {
        let conn = system_bus()?;
        let nm = nm_proxy(&conn)?;
        nm.set_property("WirelessEnabled", &_enabled)
            .map_err(|e| BackendError::Unavailable(e.to_string()))
    }

    fn request_scan(&self) -> BackendResult<()> {
        let conn = system_bus()?;
        let nm = nm_proxy(&conn)?;
        let wifi_device = first_wifi_device(&conn, &nm)?;
        let wireless = wireless_proxy(&conn, &wifi_device)?;
        let options: HashMap<&str, zbus::zvariant::Value> = HashMap::new();
        wireless
            .call("RequestScan", &(options))
            .map_err(|e| BackendError::Unavailable(e.to_string()))
    }

    fn connect_network(&self, _ssid: &str, _password: Option<&str>) -> BackendResult<()> {
        Err(BackendError::NotImplemented)
    }

    fn connect_hidden(
        &self,
        _ssid: &str,
        _security: &str,
        _password: Option<&str>,
    ) -> BackendResult<()> {
        Err(BackendError::NotImplemented)
    }

    fn set_ip_dns(
        &self,
        _ssid: &str,
        _ip: Option<&str>,
        _dns: Option<&str>,
    ) -> BackendResult<()> {
        Err(BackendError::NotImplemented)
    }

    fn get_saved_password(&self, _ssid: &str) -> BackendResult<Option<String>> {
        Err(BackendError::NotImplemented)
    }

    fn set_autoreconnect(&self, _ssid: &str, _enabled: bool) -> BackendResult<()> {
        Err(BackendError::NotImplemented)
    }
}

pub mod nm_consts {
    pub const BUS_NAME: &str = "org.freedesktop.NetworkManager";
    pub const OBJECT_PATH: &str = "/org/freedesktop/NetworkManager";
    pub const DEVICE_INTERFACE: &str = "org.freedesktop.NetworkManager.Device";
    pub const WIFI_DEVICE_INTERFACE: &str = "org.freedesktop.NetworkManager.Device.Wireless";
    pub const AP_INTERFACE: &str = "org.freedesktop.NetworkManager.AccessPoint";
    pub const SETTINGS_INTERFACE: &str = "org.freedesktop.NetworkManager.Settings";
    pub const CONNECTION_INTERFACE: &str = "org.freedesktop.NetworkManager.Settings.Connection";
}

const NM_DEVICE_TYPE_WIFI: u32 = 2;

fn system_bus() -> BackendResult<Connection> {
    Connection::system().map_err(|e| BackendError::Unavailable(e.to_string()))
}

fn nm_proxy(conn: &Connection) -> BackendResult<Proxy<'_>> {
    Proxy::new(conn, nm_consts::BUS_NAME, nm_consts::OBJECT_PATH, "org.freedesktop.NetworkManager")
        .map_err(|e| BackendError::Unavailable(e.to_string()))
}

fn device_proxy<'a>(
    conn: &'a Connection,
    path: &'a OwnedObjectPath,
) -> BackendResult<Proxy<'a>> {
    Proxy::new(conn, nm_consts::BUS_NAME, path.as_str(), nm_consts::DEVICE_INTERFACE)
        .map_err(|e| BackendError::Unavailable(e.to_string()))
}

fn wireless_proxy<'a>(
    conn: &'a Connection,
    path: &'a OwnedObjectPath,
) -> BackendResult<Proxy<'a>> {
    Proxy::new(conn, nm_consts::BUS_NAME, path.as_str(), nm_consts::WIFI_DEVICE_INTERFACE)
        .map_err(|e| BackendError::Unavailable(e.to_string()))
}

fn ap_proxy<'a>(conn: &'a Connection, path: &'a OwnedObjectPath) -> BackendResult<Proxy<'a>> {
    Proxy::new(conn, nm_consts::BUS_NAME, path.as_str(), nm_consts::AP_INTERFACE)
        .map_err(|e| BackendError::Unavailable(e.to_string()))
}

fn first_wifi_device(conn: &Connection, nm: &Proxy<'_>) -> BackendResult<OwnedObjectPath> {
    let devices: Vec<OwnedObjectPath> = nm
        .call("GetDevices", &())
        .map_err(|e| BackendError::Unavailable(e.to_string()))?;

    for path in devices {
        let device_type: u32 = {
            let device = device_proxy(conn, &path)?;
            device
                .get_property("DeviceType")
                .map_err(|e| BackendError::Unavailable(e.to_string()))?
        };
        if device_type == NM_DEVICE_TYPE_WIFI {
            return Ok(path);
        }
    }

    Err(BackendError::Unavailable(
        "No Wi‑Fi device found".to_string(),
    ))
}

fn icon_for_strength(strength: u8) -> &'static str {
    match strength {
        0..=20 => "network-wireless-signal-none",
        21..=40 => "network-wireless-signal-weak",
        41..=60 => "network-wireless-signal-ok",
        61..=80 => "network-wireless-signal-good",
        _ => "network-wireless-signal-excellent",
    }
}
