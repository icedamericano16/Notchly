//! Core Audio: master volume / mute callbacks, mic mute, default-device changes
//! and per-app volume (ISimpleAudioVolume matched by process).

use std::sync::mpsc::{channel, Sender};

use windows::core::{implement, Interface, PCWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::{CloseHandle, PROPERTYKEY};
use windows::Win32::Media::Audio::Endpoints::*;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::StructuredStorage::{
    PropVariantToStringAlloc, PropVariantToUInt32,
};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, STGM_READ};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

use super::{com_mta, spawn, Bus, Event};
use crate::util::log;

#[derive(Debug, Clone)]
pub enum AudioCmd {
    SetVolume(f32),
    Step(f32),
    ToggleMicMute,
    /// Set volume of the audio session owned by an executable (lowercase file name).
    SetAppVolume(String, f32),
    QueryAppVolume(String),
    Rebind,
}

#[implement(IAudioEndpointVolumeCallback)]
struct VolCb {
    bus: Bus,
    mic: bool,
}

impl IAudioEndpointVolumeCallback_Impl for VolCb_Impl {
    fn OnNotify(&self, data: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        if data.is_null() {
            return Ok(());
        }
        let d = unsafe { &*data };
        if self.mic {
            self.bus.send(Event::Mic {
                muted: d.bMuted.as_bool(),
            });
        } else {
            self.bus.send(Event::Volume {
                level: d.fMasterVolume,
                muted: d.bMuted.as_bool(),
            });
        }
        Ok(())
    }
}

#[implement(IMMNotificationClient)]
struct DevCb {
    tx: Sender<AudioCmd>,
}

impl IMMNotificationClient_Impl for DevCb_Impl {
    fn OnDeviceStateChanged(
        &self,
        _id: &PCWSTR,
        _state: DEVICE_STATE,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnDeviceAdded(&self, _id: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnDeviceRemoved(&self, _id: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnDefaultDeviceChanged(
        &self,
        _flow: EDataFlow,
        role: ERole,
        _id: &PCWSTR,
    ) -> windows::core::Result<()> {
        if role == eMultimedia || role == eCommunications && _flow == eCapture {
            let _ = self.tx.send(AudioCmd::Rebind);
        }
        Ok(())
    }
    fn OnPropertyValueChanged(
        &self,
        _id: &PCWSTR,
        _key: &PROPERTYKEY,
    ) -> windows::core::Result<()> {
        Ok(())
    }
}

struct Bound {
    vol: Option<IAudioEndpointVolume>,
    vol_cb: Option<IAudioEndpointVolumeCallback>,
    mic: Option<IAudioEndpointVolume>,
    mic_cb: Option<IAudioEndpointVolumeCallback>,
    render_id: String,
}

impl Bound {
    fn unbind(&mut self) {
        unsafe {
            if let (Some(v), Some(cb)) = (&self.vol, &self.vol_cb) {
                let _ = v.UnregisterControlChangeNotify(cb);
            }
            if let (Some(v), Some(cb)) = (&self.mic, &self.mic_cb) {
                let _ = v.UnregisterControlChangeNotify(cb);
            }
        }
        self.vol = None;
        self.mic = None;
    }
}

pub fn start(bus: Bus) -> Sender<AudioCmd> {
    let (tx, rx) = channel::<AudioCmd>();
    let tx_dev = tx.clone();
    spawn("audio", move || {
        com_mta();
        let en: IMMDeviceEnumerator =
            match unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) } {
                Ok(e) => e,
                Err(e) => {
                    log(&format!("audio enumerator failed: {e}"));
                    return;
                }
            };
        let dev_cb: IMMNotificationClient = DevCb { tx: tx_dev }.into();
        unsafe {
            let _ = en.RegisterEndpointNotificationCallback(&dev_cb);
        }
        let mut b = Bound {
            vol: None,
            vol_cb: None,
            mic: None,
            mic_cb: None,
            render_id: String::new(),
        };
        unsafe { bind(&en, &mut b, &bus, true) };
        while let Ok(cmd) = rx.recv() {
            unsafe {
                match cmd {
                    AudioCmd::Rebind => bind(&en, &mut b, &bus, false),
                    AudioCmd::SetVolume(v) => {
                        if let Some(vol) = &b.vol {
                            let _ =
                                vol.SetMasterVolumeLevelScalar(v.clamp(0.0, 1.0), std::ptr::null());
                            if v > 0.0 && vol.GetMute().map(|m| m.as_bool()).unwrap_or(false) {
                                let _ = vol.SetMute(false, std::ptr::null());
                            }
                        }
                    }
                    AudioCmd::Step(d) => {
                        if let Some(vol) = &b.vol {
                            let cur = vol.GetMasterVolumeLevelScalar().unwrap_or(0.0);
                            let _ = vol.SetMasterVolumeLevelScalar(
                                (cur + d).clamp(0.0, 1.0),
                                std::ptr::null(),
                            );
                        }
                    }
                    AudioCmd::ToggleMicMute => {
                        if let Some(mic) = &b.mic {
                            let m = mic.GetMute().map(|m| m.as_bool()).unwrap_or(false);
                            let _ = mic.SetMute(!m, std::ptr::null());
                        }
                    }
                    AudioCmd::SetAppVolume(exe, v) => {
                        let level = app_volume(&en, &exe, Some(v));
                        bus.send(Event::AppVolume { level });
                    }
                    AudioCmd::QueryAppVolume(exe) => {
                        let level = app_volume(&en, &exe, None);
                        bus.send(Event::AppVolume { level });
                    }
                }
            }
        }
    });
    tx
}

unsafe fn bind(en: &IMMDeviceEnumerator, b: &mut Bound, bus: &Bus, initial: bool) {
    b.unbind();
    if let Ok(dev) = en.GetDefaultAudioEndpoint(eRender, eMultimedia) {
        let id = dev.GetId().ok().map(|p| {
            let s = p.to_string().unwrap_or_default();
            CoTaskMemFree(Some(p.0 as _));
            s
        });
        if let Ok(vol) = dev.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
            let cb: IAudioEndpointVolumeCallback = VolCb {
                bus: bus.clone(),
                mic: false,
            }
            .into();
            let _ = vol.RegisterControlChangeNotify(&cb);
            let level = vol.GetMasterVolumeLevelScalar().unwrap_or(0.0);
            let muted = vol.GetMute().map(|m| m.as_bool()).unwrap_or(false);
            // initial state is sent as a quiet sync (the UI ignores start-up events)
            bus.send(Event::Volume { level, muted });
            b.vol = Some(vol);
            b.vol_cb = Some(cb);
        }
        let id = id.unwrap_or_default();
        if id != b.render_id {
            let (name, headphones) = device_info(&dev);
            bus.send(Event::AudioDevice {
                name,
                headphones,
                initial: initial || b.render_id.is_empty(),
            });
            b.render_id = id;
        }
    }
    if let Ok(dev) = en.GetDefaultAudioEndpoint(eCapture, eCommunications) {
        if let Ok(mic) = dev.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
            let cb: IAudioEndpointVolumeCallback = VolCb {
                bus: bus.clone(),
                mic: true,
            }
            .into();
            let _ = mic.RegisterControlChangeNotify(&cb);
            b.mic = Some(mic);
            b.mic_cb = Some(cb);
        }
    }
}

unsafe fn device_info(dev: &IMMDevice) -> (String, bool) {
    let Ok(store) = dev.OpenPropertyStore(STGM_READ) else {
        return ("Speakers".into(), false);
    };
    let name = store
        .GetValue(&PKEY_Device_FriendlyName)
        .ok()
        .and_then(|pv| PropVariantToStringAlloc(&pv).ok())
        .map(|p| {
            let s = p.to_string().unwrap_or_default();
            CoTaskMemFree(Some(p.0 as _));
            s
        })
        .unwrap_or_else(|| "Speakers".into());
    let ff = store
        .GetValue(&PKEY_AudioEndpoint_FormFactor)
        .ok()
        .and_then(|pv| PropVariantToUInt32(&pv).ok())
        .unwrap_or(0);
    // Headphones = 3, Headset = 5
    let headphones = ff == 3
        || ff == 5
        || name.to_lowercase().contains("head")
        || name.to_lowercase().contains("buds")
        || name.to_lowercase().contains("airpods");
    (name, headphones)
}

/// Gets (and optionally sets) the volume of the first session owned by `exe`.
unsafe fn app_volume(en: &IMMDeviceEnumerator, exe: &str, set: Option<f32>) -> Option<f32> {
    let dev = en.GetDefaultAudioEndpoint(eRender, eMultimedia).ok()?;
    let mgr: IAudioSessionManager2 = dev.Activate(CLSCTX_ALL, None).ok()?;
    let list = mgr.GetSessionEnumerator().ok()?;
    let n = list.GetCount().ok()?;
    let mut found = None;
    for i in 0..n {
        let Ok(ctl) = list.GetSession(i) else {
            continue;
        };
        let Ok(ctl2) = ctl.cast::<IAudioSessionControl2>() else {
            continue;
        };
        let Ok(pid) = ctl2.GetProcessId() else {
            continue;
        };
        if pid == 0 || !process_name(pid).eq_ignore_ascii_case(exe) {
            continue;
        }
        let Ok(sv) = ctl.cast::<ISimpleAudioVolume>() else {
            continue;
        };
        if let Some(v) = set {
            let _ = sv.SetMasterVolume(v.clamp(0.0, 1.0), std::ptr::null());
        }
        found = sv.GetMasterVolume().ok();
    }
    found
}

/// Executable file name of a process id (e.g. "spotify.exe").
pub fn process_name(pid: u32) -> String {
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::new();
        };
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            h,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(h);
        if !ok {
            return String::new();
        }
        let full = String::from_utf16_lossy(&buf[..len as usize]);
        full.rsplit('\\').next().unwrap_or("").to_string()
    }
}

/// Maps a media AUMID to the executable that owns its audio session.
pub fn exe_for_app(aumid: &str) -> String {
    let l = aumid.to_lowercase();
    if l.ends_with(".exe") {
        return l.rsplit('\\').next().unwrap_or(&l).to_string();
    }
    for (k, v) in [
        ("spotify", "spotify.exe"),
        ("msedge", "msedge.exe"),
        ("chrome", "chrome.exe"),
        ("firefox", "firefox.exe"),
        ("308046b0af4a39cb", "firefox.exe"),
        ("zunemusic", "microsoft.media.player.exe"),
        ("vlc", "vlc.exe"),
        ("brave", "brave.exe"),
        ("opera", "opera.exe"),
    ] {
        if l.contains(k) {
            return v.into();
        }
    }
    format!("{l}.exe")
}
