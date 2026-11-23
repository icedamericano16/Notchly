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
    /// All radios off (remembering which were on), or back as they were.
    SetAirplane(bool),
}

/// Current switch positions; `None` when the PC has no such radio (or access is denied).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Radios {
    pub wifi: Option<bool>,
    pub bluetooth: Option<bool>,
    /// Windows' airplane mode, or every radio switched off from here.
    pub airplane: Option<bool>,
}

/// Windows' own airplane mode switch (read-only; HKLM RadioManagement).
fn system_airplane() -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::*;
    let k = crate::util::wide(r"SYSTEM\CurrentControlSet\Control\RadioManagement\SystemRadioState");
    let mut v = 0u32;
    let mut size = 4u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(k.as_ptr()),
            PCWSTR::null(),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut v as *mut u32 as _),
            Some(&mut size),
        )
    };
    r.is_ok() && v == 1
}

fn list() -> Vec<Radio> {
    let Ok(v) = Radio::GetRadiosAsync().and_then(|o| o.join()) else {
        return Vec::new();
    };
    (0..v.Size().unwrap_or(0))
        .filter_map(|i| v.GetAt(i).ok())
        .collect()
}

fn read(radios: &[Radio], ours: bool) -> Radios {
    let state = |kind: RadioKind| {
        radios
            .iter()
            .find(|r| r.Kind().is_ok_and(|k| k == kind))
            .and_then(|r| r.State().ok())
            .map(|s| s == RadioState::On)
    };
    let all_off = radios
        .iter()
        .all(|r| r.State().is_ok_and(|s| s != RadioState::On));
    Radios {
        wifi: state(RadioKind::WiFi),
        bluetooth: state(RadioKind::Bluetooth),
        airplane: (!radios.is_empty()).then(|| system_airplane() || (ours && all_off)),
    }
}

/// Every radio off, remembering which were on.
fn airplane_on(radios: &[Radio]) -> (Vec<RadioKind>, Result<(), String>) {
    let mut was_on = Vec::new();
    let mut res = Ok(());
    for r in radios {
        if r.State().is_ok_and(|s| s == RadioState::On) {
            if let Ok(k) = r.Kind() {
                was_on.push(k);
            }
            if let Err(e) = r.SetStateAsync(RadioState::Off).and_then(|o| o.join()) {
                res = Err(e.to_string());
            }
        }
    }
    (was_on, res)
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
        // radios switched off by our airplane mode (restored when it's turned off)
        let mut ours: Option<Vec<RadioKind>> = None;
        bus.send(Event::Radios(read(&radios, false)));
        while let Ok(cmd) = rx.recv() {
            if radios.is_empty() {
                radios = list();
            }
            let result = match cmd {
                RadioCmd::Refresh => {
                    radios = list();
                    Ok(())
                }
                RadioCmd::SetWifi(on) => {
                    ours = None;
                    set(&radios, RadioKind::WiFi, on)
                }
                RadioCmd::SetBluetooth(on) => {
                    ours = None;
                    set(&radios, RadioKind::Bluetooth, on)
                }
                RadioCmd::SetAirplane(true) => {
                    let (was, r) = airplane_on(&radios);
                    ours = Some(was);
                    r
                }
                RadioCmd::SetAirplane(false) => {
                    // back as they were; Wi-Fi if we don't know
                    let kinds = ours
                        .take()
                        .filter(|k| !k.is_empty())
                        .unwrap_or_else(|| vec![RadioKind::WiFi]);
                    let mut r = Ok(());
                    for k in kinds {
                        if let Err(e) = set(&radios, k, true) {
                            r = Err(e);
                        }
                    }
                    r
                }
            };
            if let Err(e) = result {
                bus.send(Event::Notice(false, "Couldn't change the radio".into(), e));
            }
            bus.send(Event::Radios(read(&radios, ours.is_some())));
        }
    });
    tx
}
