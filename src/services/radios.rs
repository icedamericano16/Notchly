//! Wi-Fi and Bluetooth radio switches (Windows.Devices.Radios) for the Control Center.
//! A worker owns the WinRT objects; the UI sends commands and receives state events.

use std::sync::mpsc::{channel, Sender};

use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioKind, RadioState};

use super::{com_mta, spawn, Bus, Event};
use crate::util::log;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadioCmd {
    Refresh,
    SetWifi(bool),
    SetBluetooth(bool),
}

/// Current switch positions; `None` when the PC has no such radio (or access is denied).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Radios {
    pub wifi: Option<bool>,
    pub bluetooth: Option<bool>,
}

fn list() -> Vec<Radio> {
    let Ok(v) = Radio::GetRadiosAsync().and_then(|o| o.join()) else {
        return Vec::new();
    };
    (0..v.Size().unwrap_or(0))
        .filter_map(|i| v.GetAt(i).ok())
        .collect()
}

fn read(radios: &[Radio]) -> Radios {
    let state = |kind: RadioKind| {
        radios
            .iter()
            .find(|r| r.Kind().is_ok_and(|k| k == kind))
            .and_then(|r| r.State().ok())
            .map(|s| s == RadioState::On)
    };
    Radios {
        wifi: state(RadioKind::WiFi),
        bluetooth: state(RadioKind::Bluetooth),
    }
}

fn set(radios: &[Radio], kind: RadioKind, on: bool) -> Result<(), String> {
    let r = radios
        .iter()
        .find(|r| r.Kind().is_ok_and(|k| k == kind))
        .ok_or("This PC has no such radio")?;
    let target = if on { RadioState::On } else { RadioState::Off };
    match r.SetStateAsync(target).and_then(|o| o.join()) {
        Ok(RadioAccessStatus::Allowed) => Ok(()),
        Ok(RadioAccessStatus::DeniedByUser) => Err("Turned off in Windows privacy settings".into()),
        Ok(_) => Err("Windows didn't allow the change".into()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn start(bus: Bus) -> Sender<RadioCmd> {
    let (tx, rx) = channel::<RadioCmd>();
    spawn("radios", move || {
        com_mta();
        let access = Radio::RequestAccessAsync()
            .and_then(|o| o.join())
            .map(|a| a == RadioAccessStatus::Allowed)
            .unwrap_or(false);
        if !access {
            log("radios: access not granted");
        }
        let mut radios = list();
        bus.send(Event::Radios(read(&radios)));
        while let Ok(cmd) = rx.recv() {
            if radios.is_empty() {
                radios = list();
            }
            let result = match cmd {
                RadioCmd::Refresh => {
                    radios = list();
                    Ok(())
                }
                RadioCmd::SetWifi(on) => set(&radios, RadioKind::WiFi, on),
                RadioCmd::SetBluetooth(on) => set(&radios, RadioKind::Bluetooth, on),
            };
            if let Err(e) = result {
                bus.send(Event::Notice(false, "Couldn't change the radio".into(), e));
            }
            bus.send(Event::Radios(read(&radios)));
        }
    });
    tx
}
