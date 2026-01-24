#[derive(Clone, Debug)]
pub enum NetworkAction {
    None,
    Connect,
    Disconnect,
}

#[derive(Clone, Debug)]
pub struct Network {
    pub ssid: String,
    pub signal_icon: &'static str,
    pub action: NetworkAction,
    pub strength: u8,
    pub is_active: bool,
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub wifi_enabled: bool,
    pub networks: Vec<Network>,
}

#[derive(Clone, Debug, Default)]
pub struct NetworkDetails {
    pub ip_address: Option<String>,
    pub dns_server: Option<String>,
    pub auto_reconnect: Option<bool>,
}
