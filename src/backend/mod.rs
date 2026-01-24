pub mod nm;

use crate::models::{AppState, NetworkDetails};

#[derive(Debug)]
pub enum BackendError {
    NotImplemented,
    Unavailable(String),
    PermissionDenied,
}

pub type BackendResult<T> = Result<T, BackendError>;

pub trait Backend {
    fn load_state(&self) -> BackendResult<AppState>;
    fn set_wifi_enabled(&self, enabled: bool) -> BackendResult<()>;
    fn request_scan(&self) -> BackendResult<()>;
    fn connect_network(&self, ssid: &str, password: Option<&str>) -> BackendResult<()>;
    fn disconnect_network(&self, ssid: &str) -> BackendResult<()>;
    fn connect_hidden(
        &self,
        ssid: &str,
        security: &str,
        password: Option<&str>,
    ) -> BackendResult<()>;
    fn get_network_details(&self, ssid: &str) -> BackendResult<NetworkDetails>;
    fn set_ip_dns(
        &self,
        ssid: &str,
        ip: Option<&str>,
        prefix: Option<u32>,
        gateway: Option<&str>,
        dns: Option<Vec<String>>,
    ) -> BackendResult<()>;
    fn get_saved_password(&self, ssid: &str) -> BackendResult<Option<String>>;
    fn set_autoreconnect(&self, ssid: &str, enabled: bool) -> BackendResult<()>;
    fn forget_network(&self, ssid: &str) -> BackendResult<()>;
}
