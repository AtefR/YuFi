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
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub wifi_enabled: bool,
    pub networks: Vec<Network>,
}
