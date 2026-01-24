use crate::backend::{Backend, BackendError, BackendResult};
use crate::models::{AppState, Network, NetworkAction, NetworkDetails};
use std::collections::HashMap;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{Array, OwnedObjectPath, OwnedValue, Str};

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
        let active_ssid = active_ssid_for_device(&conn, &wifi_device)?;

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

            let is_active =
                ap_path == active_ap || active_ssid.as_deref().is_some_and(|v| v == ssid);
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
        let conn = system_bus()?;
        let nm = nm_proxy(&conn)?;
        let wifi_device = first_wifi_device(&conn, &nm)?;
        let wireless = wireless_proxy(&conn, &wifi_device)?;

        let (ap_path, _ap_strength) = find_ap_for_ssid(&conn, &wireless, _ssid)?;

        let settings = nm_settings_proxy(&conn)?;
        if let Some(connection_path) = find_connection_for_ssid(&conn, &settings, _ssid)? {
            let _: OwnedObjectPath = nm
                .call(
                    "ActivateConnection",
                    &(connection_path, wifi_device.clone(), ap_path),
                )
                .map_err(|e| BackendError::Unavailable(e.to_string()))?;
            return Ok(());
        }

        let mut connection: HashMap<String, HashMap<String, OwnedValue>> = HashMap::new();
        let mut con_section = HashMap::new();
        con_section.insert("type".to_string(), ov_str("802-11-wireless"));
        con_section.insert("id".to_string(), ov_str(_ssid));
        con_section.insert("autoconnect".to_string(), OwnedValue::from(true));
        connection.insert("connection".to_string(), con_section);

        let mut wifi_section = HashMap::new();
        wifi_section.insert("ssid".to_string(), ov_bytes(_ssid.as_bytes().to_vec())?);
        wifi_section.insert("mode".to_string(), ov_str("infrastructure"));
        connection.insert("802-11-wireless".to_string(), wifi_section);

        if let Some(password) = _password {
            let mut sec_section = HashMap::new();
            sec_section.insert("key-mgmt".to_string(), ov_str("wpa-psk"));
            sec_section.insert("psk".to_string(), ov_str(password));
            connection.insert("802-11-wireless-security".to_string(), sec_section);
        }

        let _: (OwnedObjectPath, OwnedObjectPath) = nm
            .call(
                "AddAndActivateConnection",
                &(connection, wifi_device.clone(), ap_path),
            )
            .map_err(|e| BackendError::Unavailable(e.to_string()))?;

        Ok(())
    }

    fn disconnect_network(&self, ssid: &str) -> BackendResult<()> {
        let conn = system_bus()?;
        let nm = nm_proxy(&conn)?;
        let active_path = find_active_connection_for_ssid(&conn, &nm, ssid)?
            .ok_or_else(|| BackendError::Unavailable("No active connection".to_string()))?;
        let _: () = nm
            .call("DeactivateConnection", &(active_path))
            .map_err(|e| BackendError::Unavailable(e.to_string()))?;
        Ok(())
    }

    fn connect_hidden(
        &self,
        ssid: &str,
        _security: &str,
        password: Option<&str>,
    ) -> BackendResult<()> {
        let conn = system_bus()?;
        let nm = nm_proxy(&conn)?;
        let wifi_device = first_wifi_device(&conn, &nm)?;

        let settings = nm_settings_proxy(&conn)?;
        if let Some(connection_path) = find_connection_for_ssid(&conn, &settings, ssid)? {
            let ap = OwnedObjectPath::try_from("/")
                .map_err(|e| BackendError::Unavailable(e.to_string()))?;
            let _: OwnedObjectPath = nm
                .call("ActivateConnection", &(connection_path, wifi_device, ap))
                .map_err(|e| BackendError::Unavailable(e.to_string()))?;
            return Ok(());
        }

        let mut connection: HashMap<String, HashMap<String, OwnedValue>> = HashMap::new();
        let mut con_section = HashMap::new();
        con_section.insert("type".to_string(), ov_str("802-11-wireless"));
        con_section.insert("id".to_string(), ov_str(ssid));
        con_section.insert("autoconnect".to_string(), OwnedValue::from(true));
        connection.insert("connection".to_string(), con_section);

        let mut wifi_section = HashMap::new();
        wifi_section.insert("ssid".to_string(), ov_bytes(ssid.as_bytes().to_vec())?);
        wifi_section.insert("mode".to_string(), ov_str("infrastructure"));
        wifi_section.insert("hidden".to_string(), OwnedValue::from(true));
        connection.insert("802-11-wireless".to_string(), wifi_section);

        if let Some(password) = password {
            let mut sec_section = HashMap::new();
            sec_section.insert("key-mgmt".to_string(), ov_str("wpa-psk"));
            sec_section.insert("psk".to_string(), ov_str(password));
            connection.insert("802-11-wireless-security".to_string(), sec_section);
        }

        let ap_path = OwnedObjectPath::try_from("/").map_err(|e| BackendError::Unavailable(e.to_string()))?;
        let _: (OwnedObjectPath, OwnedObjectPath) = nm
            .call("AddAndActivateConnection", &(connection, wifi_device.clone(), ap_path))
            .map_err(|e| BackendError::Unavailable(e.to_string()))?;

        Ok(())
    }

    fn get_network_details(&self, ssid: &str) -> BackendResult<NetworkDetails> {
        let conn = system_bus()?;
        let settings = nm_settings_proxy(&conn)?;
        let connection_path = find_connection_for_ssid(&conn, &settings, ssid)?
            .ok_or_else(|| BackendError::Unavailable("Connection not found".to_string()))?;

        let settings_map = connection_settings(&conn, &connection_path)?;

        let mut details = NetworkDetails::default();

        if let Some(connection) = settings_map.get("connection") {
            if let Some(value) = connection.get("autoconnect") {
                if let Ok(flag) = owned_value_to_bool(value) {
                    details.auto_reconnect = Some(flag);
                }
            }
        }

        if let Some(ipv4) = settings_map.get("ipv4") {
            if let Some(value) = ipv4.get("address-data") {
                if let Some((addr, prefix)) = first_address_from_value(value) {
                    details.ip_address = Some(addr);
                    details.prefix = Some(prefix);
                }
            }
            if let Some(value) = ipv4.get("gateway") {
                if let Ok(gateway) = owned_value_to_string(value) {
                    details.gateway = Some(gateway);
                }
            }
            if let Some(value) = ipv4.get("dns-data") {
                details.dns_servers = dns_from_value(value);
            }
        }

        Ok(details)
    }

    fn set_ip_dns(
        &self,
        ssid: &str,
        ip: Option<&str>,
        prefix: Option<u32>,
        gateway: Option<&str>,
        dns: Option<Vec<String>>,
    ) -> BackendResult<()> {
        if ip.is_none() && dns.is_none() && gateway.is_none() {
            return Ok(());
        }

        let conn = system_bus()?;
        let settings = nm_settings_proxy(&conn)?;
        let connection_path = find_connection_for_ssid(&conn, &settings, ssid)?
            .ok_or_else(|| BackendError::Unavailable("Connection not found".to_string()))?;

        let mut settings_map = connection_settings(&conn, &connection_path)?;
        let ipv4 = settings_map
            .entry("ipv4".to_string())
            .or_insert_with(HashMap::new);

        let mut set_manual = false;

        if let Some(ip) = ip {
            let (address, default_prefix) = parse_ip_prefix(ip);
            let prefix = prefix.unwrap_or(default_prefix);
            ipv4.insert("method".to_string(), ov_str("manual"));
            let mut addr = HashMap::new();
            addr.insert("address".to_string(), ov_str(&address));
            addr.insert("prefix".to_string(), OwnedValue::from(prefix));
            let address_data = vec![addr];
            ipv4.insert("address-data".to_string(), ov_array_dict(address_data)?);
            set_manual = true;
        }

        if let Some(gateway) = gateway {
            ipv4.insert("gateway".to_string(), ov_str(gateway));
            set_manual = true;
        }

        if let Some(dns_list) = dns {
            let mut dns_data = Vec::new();
            for dns in dns_list {
                if dns.trim().is_empty() {
                    continue;
                }
                let mut dns_entry = HashMap::new();
                dns_entry.insert("address".to_string(), ov_str(dns.trim()));
                dns_data.push(dns_entry);
            }
            if !dns_data.is_empty() {
                ipv4.insert("dns-data".to_string(), ov_array_dict(dns_data)?);
                ipv4.insert("ignore-auto-dns".to_string(), OwnedValue::from(true));
                set_manual = true;
            }
        }

        if set_manual {
            ipv4.insert("method".to_string(), ov_str("manual"));
        }

        update_connection(&conn, &connection_path, settings_map)
    }

    fn get_saved_password(&self, _ssid: &str) -> BackendResult<Option<String>> {
        let conn = system_bus()?;
        let settings = nm_settings_proxy(&conn)?;
        let connection_path = find_connection_for_ssid(&conn, &settings, _ssid)?
            .ok_or_else(|| BackendError::Unavailable("Connection not found".to_string()))?;

        let connection_proxy = connection_proxy(&conn, &connection_path)?;
        let secrets: HashMap<String, HashMap<String, OwnedValue>> = connection_proxy
            .call("GetSecrets", &("802-11-wireless-security",))
            .map_err(|e| BackendError::Unavailable(e.to_string()))?;

        let sec = match secrets.get("802-11-wireless-security") {
            Some(section) => section,
            None => return Ok(None),
        };

        if let Some(value) = sec.get("psk") {
            return owned_value_to_string(value).map(Some);
        }
        if let Some(value) = sec.get("wep-key0") {
            return owned_value_to_string(value).map(Some);
        }

        Ok(None)
    }

    fn set_autoreconnect(&self, _ssid: &str, _enabled: bool) -> BackendResult<()> {
        let conn = system_bus()?;
        let settings = nm_settings_proxy(&conn)?;
        let connection_path = find_connection_for_ssid(&conn, &settings, _ssid)?
            .ok_or_else(|| BackendError::Unavailable("Connection not found".to_string()))?;

        let mut settings_map = connection_settings(&conn, &connection_path)?;
        let connection = settings_map
            .entry("connection".to_string())
            .or_insert_with(HashMap::new);
        connection.insert("autoconnect".to_string(), OwnedValue::from(_enabled));

        update_connection(&conn, &connection_path, settings_map)
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

fn nm_settings_proxy(conn: &Connection) -> BackendResult<Proxy<'_>> {
    Proxy::new(
        conn,
        nm_consts::BUS_NAME,
        "/org/freedesktop/NetworkManager/Settings",
        nm_consts::SETTINGS_INTERFACE,
    )
    .map_err(|e| BackendError::Unavailable(e.to_string()))
}

fn connection_proxy<'a>(
    conn: &'a Connection,
    path: &'a OwnedObjectPath,
) -> BackendResult<Proxy<'a>> {
    Proxy::new(
        conn,
        nm_consts::BUS_NAME,
        path.as_str(),
        nm_consts::CONNECTION_INTERFACE,
    )
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

fn ov_str(value: &str) -> OwnedValue {
    OwnedValue::from(Str::from(value))
}

fn ov_bytes(bytes: Vec<u8>) -> BackendResult<OwnedValue> {
    OwnedValue::try_from(Array::from(bytes))
        .map_err(|e| BackendError::Unavailable(e.to_string()))
}

fn ov_array_dict(value: Vec<HashMap<String, OwnedValue>>) -> BackendResult<OwnedValue> {
    OwnedValue::try_from(Array::from(value)).map_err(|e| BackendError::Unavailable(e.to_string()))
}

fn owned_value_to_string(value: &OwnedValue) -> BackendResult<String> {
    let owned = value
        .try_clone()
        .map_err(|e| BackendError::Unavailable(e.to_string()))?;
    String::try_from(owned).map_err(|e| BackendError::Unavailable(e.to_string()))
}

fn owned_value_to_bool(value: &OwnedValue) -> BackendResult<bool> {
    let owned = value
        .try_clone()
        .map_err(|e| BackendError::Unavailable(e.to_string()))?;
    bool::try_from(owned).map_err(|e| BackendError::Unavailable(e.to_string()))
}

fn owned_value_to_u32(value: &OwnedValue) -> BackendResult<u32> {
    let owned = value
        .try_clone()
        .map_err(|e| BackendError::Unavailable(e.to_string()))?;
    u32::try_from(owned).map_err(|e| BackendError::Unavailable(e.to_string()))
}

fn value_to_vec_dict(
    value: &OwnedValue,
) -> Option<Vec<HashMap<String, OwnedValue>>> {
    let owned = value.try_clone().ok()?;
    Vec::<HashMap<String, OwnedValue>>::try_from(owned).ok()
}

fn first_address_from_value(value: &OwnedValue) -> Option<(String, u32)> {
    let dicts = value_to_vec_dict(value)?;
    let first = dicts.into_iter().next()?;
    let address = first.get("address")?;
    let prefix = first.get("prefix")?;
    let addr = owned_value_to_string(address).ok()?;
    let pre = owned_value_to_u32(prefix).ok()?;
    Some((addr, pre))
}

fn dns_from_value(value: &OwnedValue) -> Vec<String> {
    let Some(dicts) = value_to_vec_dict(value) else {
        return Vec::new();
    };
    dicts
        .into_iter()
        .filter_map(|dict| dict.get("address").and_then(|v| owned_value_to_string(v).ok()))
        .collect()
}

fn parse_ip_prefix(input: &str) -> (String, u32) {
    if let Some((addr, prefix)) = input.split_once('/') {
        if let Ok(prefix) = prefix.parse::<u32>() {
            return (addr.to_string(), prefix);
        }
    }
    (input.to_string(), 24)
}

fn connection_settings(
    conn: &Connection,
    path: &OwnedObjectPath,
) -> BackendResult<HashMap<String, HashMap<String, OwnedValue>>> {
    let proxy = connection_proxy(conn, path)?;
    proxy
        .call("GetSettings", &())
        .map_err(|e| BackendError::Unavailable(e.to_string()))
}

fn update_connection(
    conn: &Connection,
    path: &OwnedObjectPath,
    settings: HashMap<String, HashMap<String, OwnedValue>>,
) -> BackendResult<()> {
    let proxy = connection_proxy(conn, path)?;
    let _: () = proxy
        .call("Update", &(settings,))
        .map_err(|e| BackendError::Unavailable(e.to_string()))?;
    Ok(())
}

fn ssid_from_value(value: &OwnedValue) -> Option<String> {
    let owned = value.try_clone().ok()?;
    let bytes: Vec<u8> = Vec::try_from(owned).ok()?;
    let ssid = String::from_utf8_lossy(&bytes).trim().to_string();
    if ssid.is_empty() {
        None
    } else {
        Some(ssid)
    }
}

fn find_ap_for_ssid(
    conn: &Connection,
    wireless: &Proxy<'_>,
    ssid: &str,
) -> BackendResult<(OwnedObjectPath, u8)> {
    let ap_paths: Vec<OwnedObjectPath> = wireless
        .call("GetAccessPoints", &())
        .map_err(|e| BackendError::Unavailable(e.to_string()))?;

    let mut best: Option<(OwnedObjectPath, u8)> = None;
    for ap_path in ap_paths {
        let (current_ssid, strength) = {
            let ap = ap_proxy(conn, &ap_path)?;
            let ssid_bytes: Vec<u8> = ap
                .get_property("Ssid")
                .map_err(|e| BackendError::Unavailable(e.to_string()))?;
            let current_ssid = String::from_utf8_lossy(&ssid_bytes).trim().to_string();
            let strength: u8 = ap
                .get_property("Strength")
                .map_err(|e| BackendError::Unavailable(e.to_string()))?;
            (current_ssid, strength)
        };

        if current_ssid != ssid {
            continue;
        }
        match &best {
            Some((_, best_strength)) if *best_strength >= strength => {}
            _ => best = Some((ap_path, strength)),
        }
    }

    best.ok_or_else(|| BackendError::Unavailable("SSID not found".to_string()))
}

fn find_connection_for_ssid(
    conn: &Connection,
    settings: &Proxy<'_>,
    ssid: &str,
) -> BackendResult<Option<OwnedObjectPath>> {
    let connections: Vec<OwnedObjectPath> = settings
        .call("ListConnections", &())
        .map_err(|e| BackendError::Unavailable(e.to_string()))?;

    for path in connections {
        let is_match = {
            let connection_proxy = Proxy::new(
                conn,
                nm_consts::BUS_NAME,
                path.as_str(),
                nm_consts::CONNECTION_INTERFACE,
            )
            .map_err(|e| BackendError::Unavailable(e.to_string()))?;

            let settings_map: HashMap<String, HashMap<String, OwnedValue>> = connection_proxy
                .call("GetSettings", &())
                .map_err(|e| BackendError::Unavailable(e.to_string()))?;

            if let Some(wireless) = settings_map.get("802-11-wireless") {
                if let Some(ssid_value) = wireless.get("ssid") {
                    if let Some(current_ssid) = ssid_from_value(ssid_value) {
                        current_ssid == ssid
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        };

        if is_match {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

fn find_active_connection_for_ssid(
    conn: &Connection,
    nm: &Proxy<'_>,
    ssid: &str,
) -> BackendResult<Option<OwnedObjectPath>> {
    let active: Vec<OwnedObjectPath> = nm
        .get_property("ActiveConnections")
        .map_err(|e| BackendError::Unavailable(e.to_string()))?;

    for path in active {
        let is_match = {
            let active_proxy = Proxy::new(
                conn,
                nm_consts::BUS_NAME,
                path.as_str(),
                "org.freedesktop.NetworkManager.Connection.Active",
            )
            .map_err(|e| BackendError::Unavailable(e.to_string()))?;

            let connection: OwnedObjectPath = active_proxy
                .get_property("Connection")
                .map_err(|e| BackendError::Unavailable(e.to_string()))?;

            let settings_proxy = Proxy::new(
                conn,
                nm_consts::BUS_NAME,
                connection.as_str(),
                nm_consts::CONNECTION_INTERFACE,
            )
            .map_err(|e| BackendError::Unavailable(e.to_string()))?;

            let settings_map: HashMap<String, HashMap<String, OwnedValue>> = settings_proxy
                .call("GetSettings", &())
                .map_err(|e| BackendError::Unavailable(e.to_string()))?;

            if let Some(wireless) = settings_map.get("802-11-wireless") {
                if let Some(ssid_value) = wireless.get("ssid") {
                    if let Some(current_ssid) = ssid_from_value(ssid_value) {
                        current_ssid == ssid
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        };

        if is_match {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

fn active_ssid_for_device(
    conn: &Connection,
    device_path: &OwnedObjectPath,
) -> BackendResult<Option<String>> {
    let device = device_proxy(conn, device_path)?;
    let active: OwnedObjectPath = device
        .get_property("ActiveConnection")
        .map_err(|e| BackendError::Unavailable(e.to_string()))?;

    if active.as_str() == "/" {
        return Ok(None);
    }

    let active_proxy = Proxy::new(
        conn,
        nm_consts::BUS_NAME,
        active.as_str(),
        "org.freedesktop.NetworkManager.Connection.Active",
    )
    .map_err(|e| BackendError::Unavailable(e.to_string()))?;

    let connection: OwnedObjectPath = active_proxy
        .get_property("Connection")
        .map_err(|e| BackendError::Unavailable(e.to_string()))?;

    let settings_map = connection_settings(conn, &connection)?;

    if let Some(wireless) = settings_map.get("802-11-wireless") {
        if let Some(ssid_value) = wireless.get("ssid") {
            if let Some(current_ssid) = ssid_from_value(ssid_value) {
                return Ok(Some(current_ssid));
            }
        }
    }

    if let Some(connection) = settings_map.get("connection") {
        if let Some(id_value) = connection.get("id") {
            if let Ok(id) = owned_value_to_string(id_value) {
                return Ok(Some(id));
            }
        }
    }

    Ok(None)
}
