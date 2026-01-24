use crate::backend::{Backend, BackendResult};
use crate::models::{AppState, Network, NetworkAction, NetworkDetails};

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
                    strength: 90,
                    is_active: true,
                    is_saved: true,
                    is_secure: true,
                },
                Network {
                    ssid: "Office_Main".to_string(),
                    signal_icon: "network-wireless-signal-good",
                    action: NetworkAction::None,
                    strength: 60,
                    is_active: false,
                    is_saved: true,
                    is_secure: true,
                },
                Network {
                    ssid: "Coffee_Shop_Free".to_string(),
                    signal_icon: "network-wireless-signal-good",
                    action: NetworkAction::None,
                    strength: 55,
                    is_active: false,
                    is_saved: false,
                    is_secure: false,
                },
                Network {
                    ssid: "Guest_Network".to_string(),
                    signal_icon: "network-wireless-signal-good",
                    action: NetworkAction::Connect,
                    strength: 48,
                    is_active: false,
                    is_saved: false,
                    is_secure: true,
                },
                Network {
                    ssid: "Linksys_502".to_string(),
                    signal_icon: "network-wireless-signal-none",
                    action: NetworkAction::None,
                    strength: 15,
                    is_active: false,
                    is_saved: false,
                    is_secure: false,
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

    fn disconnect_network(&self, _ssid: &str) -> BackendResult<()> {
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

    fn get_network_details(&self, _ssid: &str) -> BackendResult<NetworkDetails> {
        Ok(NetworkDetails::default())
    }

    fn set_ip_dns(
        &self,
        _ssid: &str,
        _ip: Option<&str>,
        _prefix: Option<u32>,
        _gateway: Option<&str>,
        _dns: Option<Vec<String>>,
    ) -> BackendResult<()> {
        Ok(())
    }

    fn get_saved_password(&self, _ssid: &str) -> BackendResult<Option<String>> {
        Ok(None)
    }

    fn set_autoreconnect(&self, _ssid: &str, _enabled: bool) -> BackendResult<()> {
        Ok(())
    }

    fn forget_network(&self, _ssid: &str) -> BackendResult<()> {
        Ok(())
    }
}
