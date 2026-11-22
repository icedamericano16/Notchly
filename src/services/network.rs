//! Network status via NetworkInformation.NetworkStatusChanged.

use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::Duration;

use windows::Networking::Connectivity::*;

use super::{com_mta, spawn, Bus, Event};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct NetInfo {
    pub connected: bool,
    pub internet: bool,
    pub captive: bool,
    pub wifi: bool,
    pub name: String,
    pub bars: u8,
}

pub fn read() -> NetInfo {
    let Ok(p) = NetworkInformation::GetInternetConnectionProfile() else {
        return NetInfo::default();
    };
    let level = p
        .GetNetworkConnectivityLevel()
        .unwrap_or(NetworkConnectivityLevel::None);
    let wifi = p.IsWlanConnectionProfile().unwrap_or(false);
    let mut name = p.ProfileName().map(|s| s.to_string()).unwrap_or_default();
    if wifi {
        if let Ok(ssid) = p
            .WlanConnectionProfileDetails()
            .and_then(|d| d.GetConnectedSsid())
        {
            if !ssid.is_empty() {
                name = ssid.to_string();
            }
        }
    }
    let bars = p
        .GetSignalBars()
        .and_then(|r| r.Value())
        .unwrap_or(if wifi { 3 } else { 0 });
    NetInfo {
        connected: level != NetworkConnectivityLevel::None,
        internet: level == NetworkConnectivityLevel::InternetAccess,
        captive: level == NetworkConnectivityLevel::ConstrainedInternetAccess,
        wifi,
        name,
        bars: bars.min(4),
    }
}

pub fn start(bus: Bus) {
    spawn("network", move || {
        com_mta();
        let (tx, rx) = channel::<()>();
        let handler = NetworkStatusChangedEventHandler::new(move |_| {
            let _ = tx.send(());
            Ok(())
        });
        let _token = NetworkInformation::NetworkStatusChanged(&handler);
        let mut last = read();
        bus.send(Event::Network(last.clone()));
        loop {
            match rx.recv() {
                Ok(()) => {
                    // debounce: connectivity often flaps for a moment while switching
                    loop {
                        match rx.recv_timeout(Duration::from_millis(700)) {
                            Ok(()) => continue,
                            Err(RecvTimeoutError::Timeout) => break,
                            Err(RecvTimeoutError::Disconnected) => return,
                        }
                    }
                    let now = read();
                    if now != last {
                        last = now.clone();
                        bus.send(Event::Network(now));
                    }
                }
                Err(_) => return,
            }
        }
    });
}
