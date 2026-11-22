//! Bluetooth connect/disconnect via a DeviceWatcher over paired association
//! endpoints, plus best-effort headset battery from the device node property.

use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

use windows::core::{IInspectable, Interface, HSTRING};
use windows::Devices::Enumeration::*;
use windows::Foundation::{IReference, TypedEventHandler};
use windows_collections::{IIterable, IMapView};

use super::{com_mta, spawn, Bus, Event};
use crate::util::log;

const IS_CONNECTED: &str = "System.Devices.Aep.IsConnected";
const CONTAINER: &str = "System.Devices.Aep.ContainerId";
const BATTERY: &str = "{104EA319-6EE2-4701-BD47-8DDBF425BBE5} 2";

enum Msg {
    Added(String, String, bool),
    Updated(String, Option<bool>),
}

fn bool_prop(map: &IMapView<HSTRING, IInspectable>, key: &str) -> Option<bool> {
    let v = map.Lookup(&HSTRING::from(key)).ok()?;
    v.cast::<IReference<bool>>().ok()?.Value().ok()
}

fn props(list: &[&str]) -> Option<IIterable<HSTRING>> {
    let v: Vec<HSTRING> = list.iter().map(|s| HSTRING::from(*s)).collect();
    Some(IIterable::<HSTRING>::from(v))
}

pub fn start(bus: Bus) {
    spawn("bluetooth", move || {
        com_mta();
        // classic BT + BLE paired endpoints
        let aqs = "(System.Devices.Aep.ProtocolId:=\"{e0cbf06c-cd8b-4647-bb8a-263b43f0f974}\" OR System.Devices.Aep.ProtocolId:=\"{bb7bb05e-5972-42b5-94fc-76eaa7084d49}\") AND System.Devices.Aep.IsPaired:=System.StructuredQueryType.Boolean#True";
        let Some(p) = props(&[IS_CONNECTED, CONTAINER]) else {
            return;
        };
        let watcher = match DeviceInformation::CreateWatcherWithKindAqsFilterAndAdditionalProperties(
            &HSTRING::from(aqs),
            &p,
            DeviceInformationKind::AssociationEndpoint,
        ) {
            Ok(w) => w,
            Err(e) => {
                log(&format!("bluetooth watcher unavailable: {e}"));
                return;
            }
        };
        let (tx, rx) = channel::<Msg>();
        let t = tx.clone();
        let _ = watcher.Added(&TypedEventHandler::new(
            move |_, info: windows::core::Ref<DeviceInformation>| {
                if let Some(i) = info.as_ref() {
                    let id = i.Id().map(|s| s.to_string()).unwrap_or_default();
                    let name = i.Name().map(|s| s.to_string()).unwrap_or_default();
                    let connected = i
                        .Properties()
                        .ok()
                        .and_then(|m| bool_prop(&m, IS_CONNECTED))
                        .unwrap_or(false);
                    let _ = t.send(Msg::Added(id, name, connected));
                }
                Ok(())
            },
        ));
        let t = tx.clone();
        let _ = watcher.Updated(&TypedEventHandler::new(
            move |_, upd: windows::core::Ref<DeviceInformationUpdate>| {
                if let Some(u) = upd.as_ref() {
                    let id = u.Id().map(|s| s.to_string()).unwrap_or_default();
                    let connected = u
                        .Properties()
                        .ok()
                        .and_then(|m| bool_prop(&m, IS_CONNECTED));
                    let _ = t.send(Msg::Updated(id, connected));
                }
                Ok(())
            },
        ));
        let _ = watcher.EnumerationCompleted(&TypedEventHandler::new(|_, _| Ok(())));
        if let Err(e) = watcher.Start() {
            log(&format!("bluetooth watcher start failed: {e}"));
            return;
        }
        let devices: Arc<Mutex<HashMap<String, (String, bool)>>> =
            Arc::new(Mutex::new(HashMap::new()));
        while let Ok(m) = rx.recv() {
            let mut d = devices.lock().unwrap_or_else(|e| e.into_inner());
            match m {
                Msg::Added(id, name, connected) => {
                    d.insert(id, (name, connected));
                }
                Msg::Updated(id, Some(connected)) => {
                    if let Some((name, was)) = d.get_mut(&id) {
                        if *was != connected {
                            *was = connected;
                            let battery = if connected { battery_for(&id) } else { None };
                            bus.send(Event::Bluetooth {
                                name: name.clone(),
                                connected,
                                battery,
                            });
                        }
                    }
                }
                Msg::Updated(_, None) => {}
            }
        }
    });
}

/// Reads the Bluetooth battery level from the device node in the same container.
fn battery_for(aep_id: &str) -> Option<u8> {
    let p = props(&[CONTAINER])?;
    let aep = DeviceInformation::CreateFromIdAsyncWithKindAndAdditionalProperties(
        &HSTRING::from(aep_id),
        &p,
        DeviceInformationKind::AssociationEndpoint,
    )
    .ok()?
    .join()
    .ok()?;
    let cid = aep
        .Properties()
        .ok()?
        .Lookup(&HSTRING::from(CONTAINER))
        .ok()?;
    let guid = cid
        .cast::<IReference<windows::core::GUID>>()
        .ok()?
        .Value()
        .ok()?;
    let aqs = format!("System.Devices.ContainerId:=\"{{{guid:?}}}\"");
    let bp = props(&[BATTERY])?;
    let list = DeviceInformation::FindAllAsyncWithKindAqsFilterAndAdditionalProperties(
        &HSTRING::from(aqs),
        &bp,
        DeviceInformationKind::Device,
    )
    .ok()?
    .join()
    .ok()?;
    for i in 0..list.Size().ok()? {
        let Ok(dev) = list.GetAt(i) else { continue };
        if let Ok(v) = dev
            .Properties()
            .and_then(|m| m.Lookup(&HSTRING::from(BATTERY)))
        {
            if let Ok(b) = v.cast::<IReference<u8>>().and_then(|r| r.Value()) {
                return Some(b);
            }
        }
    }
    None
}
