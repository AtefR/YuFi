use crate::backend::{Backend, BackendResult};
use crate::models::{AppState, Network, NetworkAction};

pub struct MockBackend;

impl MockBackend {
    pub fn new() -> Self {
        Self
    }

    fn mock_state(&self) -> AppState {
        AppState {
            wifi_enabled: true,
            networks: vec![
                Network {
                    ssid: "Home_Fiber_5G".to_string(),
                    signal_icon: "network-wireless-signal-excellent",
                    action: NetworkAction::Disconnect,
                },
                Network {
                    ssid: "Office_Main".to_string(),
                    signal_icon: "network-wireless-signal-good",
                    action: NetworkAction::None,
                },
                Network {
                    ssid: "Coffee_Shop_Free".to_string(),
                    signal_icon: "network-wireless-signal-good",
                    action: NetworkAction::None,
                },
                Network {
                    ssid: "Guest_Network".to_string(),
                    signal_icon: "network-wireless-signal-good",
                    action: NetworkAction::Connect,
                },
                Network {
                    ssid: "Linksys_502".to_string(),
                    signal_icon: "network-wireless-signal-none",
                    action: NetworkAction::None,
                },
            ],
        }
    }
}

impl Backend for MockBackend {
    fn load_state(&self) -> BackendResult<AppState> {
        Ok(self.mock_state())
    }

    fn set_wifi_enabled(&self, _enabled: bool) -> BackendResult<()> {
        Ok(())
    }

    fn request_scan(&self) -> BackendResult<()> {
        Ok(())
    }

    fn connect_network(&self, _ssid: &str, _password: Option<&str>) -> BackendResult<()> {
        Ok(())
    }

    fn connect_hidden(
        &self,
        _ssid: &str,
        _security: &str,
        _password: Option<&str>,
    ) -> BackendResult<()> {
        Ok(())
    }

    fn set_ip_dns(
        &self,
        _ssid: &str,
        _ip: Option<&str>,
        _dns: Option<&str>,
    ) -> BackendResult<()> {
        Ok(())
    }

    fn get_saved_password(&self, _ssid: &str) -> BackendResult<Option<String>> {
        Ok(None)
    }

    fn set_autoreconnect(&self, _ssid: &str, _enabled: bool) -> BackendResult<()> {
        Ok(())
    }
}
