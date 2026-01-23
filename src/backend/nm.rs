use crate::backend::{Backend, BackendError, BackendResult};
use crate::models::AppState;

pub struct NetworkManagerBackend;

impl NetworkManagerBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Backend for NetworkManagerBackend {
    fn load_state(&self) -> BackendResult<AppState> {
        Err(BackendError::NotImplemented)
    }

    fn set_wifi_enabled(&self, _enabled: bool) -> BackendResult<()> {
        Err(BackendError::NotImplemented)
    }

    fn request_scan(&self) -> BackendResult<()> {
        Err(BackendError::NotImplemented)
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
