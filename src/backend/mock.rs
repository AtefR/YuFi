use crate::models::{AppState, Network, NetworkAction};

pub struct MockBackend;

impl MockBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn state(&self) -> AppState {
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
