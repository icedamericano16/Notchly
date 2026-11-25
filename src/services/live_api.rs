//! Live Activities API: a per-user named pipe (`\\.\pipe\notchly-live`) speaking
//! newline-delimited JSON, plus the `notchly live ...` CLI client.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::Deserialize;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{
    GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
    TOKEN_USER,
};
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::Pipes::*;
use windows::Win32::System::Threading::{
    CreateEventW, GetCurrentProcess, OpenProcessToken, WaitForSingleObject, INFINITE,
};
use windows::Win32::System::IO::{GetOverlappedResult, OVERLAPPED};

use super::{spawn, Bus, Event};
use crate::util::{log, wide};

pub const PIPE: &str = r"\\.\pipe\notchly-live";
pub const MAX_ACTIVITIES: usize = 8;
pub const MAX_UPDATES_PER_SEC: u32 = 10;
pub const MAX_ICON_BYTES: usize = 64 * 1024;
const MAX_LINE: usize = 160 * 1024;

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct ActionDef {
    pub id: String,
    pub label: String,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
pub struct LiveMsg {
    #[serde(skip)]
    pub client: u64,
    #[serde(skip)]
    pub exe: String,
    pub op: String,
    #[serde(default)]
    pub id: String,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub progress: Option<f32>,
    pub priority: Option<i32>,
    pub color: Option<String>,
    pub actions: Option<Vec<ActionDef>>,
    pub expires_in: Option<u64>,
    #[serde(rename = "final")]
    pub final_text: Option<String>,
    pub linger: Option<u64>,
}

impl LiveMsg {
    /// Validates sizes and field ranges. Never executes anything from payloads.
    pub fn validate(&mut self) -> Result<(), String> {
        if !matches!(self.op.as_str(), "start" | "update" | "end") {
            return Err(format!("unknown op '{}'", self.op));
        }
        if self.id.is_empty() || self.id.len() > 64 {
            return Err("id must be 1-64 chars".into());
        }
        let clip = |s: &mut Option<String>, n: usize| {
            if let Some(v) = s.as_mut() {
                if v.chars().count() > n {
                    *v = v.chars().take(n).collect();
                }
            }
        };
        clip(&mut self.title, 80);
        clip(&mut self.subtitle, 120);
        clip(&mut self.final_text, 120);
        if let Some(i) = &self.icon {
            if i.len() > MAX_ICON_BYTES * 4 / 3 + 64 {
                return Err("icon larger than 64 KB".into());
            }
        }
        if let Some(p) = self.progress.as_mut() {
            *p = p.clamp(0.0, 1.0);
        }
        if let Some(p) = self.priority.as_mut() {
            *p = (*p).clamp(10, 85);
        }
        if let Some(a) = self.actions.as_mut() {
            a.truncate(3);
            for x in a.iter_mut() {
                x.label = x.label.chars().take(24).collect();
                x.id = x.id.chars().take(32).collect();
            }
        }
        self.expires_in = self.expires_in.map(|e| e.min(24 * 3600));
        self.linger = self.linger.map(|l| l.min(30));
        Ok(())
    }
}

/// Minimal base64 decoder for data: URLs.
pub fn b64decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        } as u32;
        buf = buf << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

struct Client {
    handle: isize,
}

fn clients() -> &'static Mutex<HashMap<u64, Client>> {
    static C: OnceLock<Mutex<HashMap<u64, Client>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

static NEXT: AtomicU64 = AtomicU64::new(1);

fn user_sddl() -> Option<String> {
    unsafe {
        let mut tok = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut tok).ok()?;
        let mut len = 0u32;
        let _ = GetTokenInformation(tok, TokenUser, None, 0, &mut len);
        // u64 elements: TOKEN_USER holds a pointer, so the buffer must be pointer-aligned
        let mut buf = vec![0u64; (len as usize).div_ceil(8).max(1)];
        let got = GetTokenInformation(tok, TokenUser, Some(buf.as_mut_ptr() as _), len, &mut len);
        let _ = CloseHandle(tok);
        got.ok()?;
        // SAFETY: GetTokenInformation(TokenUser) succeeded and wrote a TOKEN_USER (plus the SID
        // it points to) into `buf`, which is aligned for it and outlives every use of `user`.
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut s = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut s).ok()?;
        let sid = s.to_string().ok()?;
        let _ = LocalFree(Some(HLOCAL(s.0 as _)));
        Some(format!("D:P(A;;GA;;;{sid})"))
    }
}

/// Sends an event JSON line to a connected client (e.g. an action click).
pub fn send(client: u64, json: &str) {
    let h = match clients().lock() {
        Ok(m) => m.get(&client).map(|c| c.handle),
        Err(_) => None,
    };
    let Some(h) = h else { return };
    let mut line = json.as_bytes().to_vec();
    line.push(b'\n');
    unsafe {
        let Ok(ev) = CreateEventW(None, true, false, None) else {
            return;
        };
        let mut ov = OVERLAPPED {
            hEvent: ev,
            ..Default::default()
        };
        let handle = HANDLE(h as _);
        let _ = WriteFile(handle, Some(&line), None, Some(&mut ov));
        let mut n = 0u32;
        let _ = GetOverlappedResult(handle, &ov, &mut n, true);
        let _ = CloseHandle(ev);
    }
}

pub fn start(bus: Bus) {
    spawn("live-api", move || unsafe {
        let Some(sddl) = user_sddl() else {
            log("live api: could not build security descriptor");
            return;
        };
        let w = wide(&sddl);
        let mut sd = PSECURITY_DESCRIPTOR::default();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(w.as_ptr()),
            SDDL_REVISION_1,
            &mut sd,
            None,
        )
        .is_err()
        {
            log("live api: bad SDDL");
            return;
        }
        let sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd.0,
            bInheritHandle: false.into(),
        };
        let name = wide(PIPE);
        let mut first = true;
        loop {
            let mut mode = PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED;
            if first {
                mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
            }
            let pipe = CreateNamedPipeW(
                PCWSTR(name.as_ptr()),
                mode,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                65536,
                65536,
                0,
                Some(&sa),
            );
            if pipe.is_invalid() {
                log("live api: CreateNamedPipe failed");
                return;
            }
            first = false;
            let Ok(ev) = CreateEventW(None, true, false, None) else {
                return;
            };
            let mut ov = OVERLAPPED {
                hEvent: ev,
                ..Default::default()
            };
            let connected = match ConnectNamedPipe(pipe, Some(&mut ov)) {
                Ok(()) => true,
                Err(e) if e.code() == ERROR_PIPE_CONNECTED.to_hresult() => true,
                Err(e) if e.code() == ERROR_IO_PENDING.to_hresult() => {
                    WaitForSingleObject(ev, INFINITE);
                    let mut n = 0;
                    GetOverlappedResult(pipe, &ov, &mut n, false).is_ok()
                }
                Err(_) => false,
            };
            let _ = CloseHandle(ev);
            if !connected {
                let _ = CloseHandle(pipe);
                continue;
            }
            let mut pid = 0u32;
            let _ = GetNamedPipeClientProcessId(pipe, &mut pid);
            let exe = super::audio::process_name(pid);
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut m) = clients().lock() {
                m.insert(
                    id,
                    Client {
                        handle: pipe.0 as isize,
                    },
                );
            }
            let b = bus.clone();
            let h = pipe.0 as isize;
            spawn("live-client", move || read_client(b, id, exe, h));
        }
    });
}

fn read_client(bus: Bus, id: u64, exe: String, h: isize) {
    let pipe = HANDLE(h as _);
    let mut pending: Vec<u8> = Vec::new();
    unsafe {
        let Ok(ev) = CreateEventW(None, true, false, None) else {
            return;
        };
        loop {
            let mut buf = [0u8; 8192];
            let mut ov = OVERLAPPED {
                hEvent: ev,
                ..Default::default()
            };
            let mut n = 0u32;
            let r = ReadFile(pipe, Some(&mut buf), None, Some(&mut ov));
            let ok = match r {
                Ok(()) => GetOverlappedResult(pipe, &ov, &mut n, true).is_ok(),
                Err(e) if e.code() == ERROR_IO_PENDING.to_hresult() => {
                    GetOverlappedResult(pipe, &ov, &mut n, true).is_ok()
                }
                Err(_) => false,
            };
            if !ok || n == 0 {
                break;
            }
            pending.extend_from_slice(&buf[..n as usize]);
            if pending.len() > MAX_LINE && !pending.contains(&b'\n') {
                log("live api: oversized message dropped");
                pending.clear();
            }
            while let Some(pos) = pending.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = pending.drain(..=pos).collect();
                let text = String::from_utf8_lossy(&line);
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                match serde_json::from_str::<LiveMsg>(text) {
                    Ok(mut m) => match m.validate() {
                        Ok(()) => {
                            m.client = id;
                            m.exe = exe.clone();
                            bus.send(Event::Live(m));
                        }
                        Err(e) => send(id, &serde_json::json!({ "event": "error", "message": e }).to_string()),
                    },
                    Err(e) => send(id, &serde_json::json!({ "event": "error", "message": format!("bad json: {e}") }).to_string()),
                }
            }
        }
        let _ = CloseHandle(ev);
        if let Ok(mut m) = clients().lock() {
            m.remove(&id);
        }
        let _ = DisconnectNamedPipe(pipe);
        let _ = CloseHandle(pipe);
    }
    bus.send(Event::LiveClientGone(id));
}

// ---------------------------------------------------------------- CLI

const CLI_HELP: &str =
    "usage: notchly live <start|update|end> --id ID [--title T] [--subtitle S] [--icon NAME]
          [--progress 0..1] [--priority 10..85] [--color #RRGGBB] [--action id:Label]...
          [--expires SECONDS] [--final TEXT] [--linger SECONDS] [--wait]
  --wait keeps the connection open and prints action events as JSON lines.";

/// Runs `notchly live ...`. Returns the process exit code.
pub fn cli(args: &[String]) -> i32 {
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        println!("{CLI_HELP}");
        return 0;
    }
    let mut obj = serde_json::Map::new();
    obj.insert("op".into(), args[0].clone().into());
    let mut actions = Vec::new();
    let mut wait = false;
    let mut i = 1;
    while i < args.len() {
        let k = args[i].as_str();
        let v = args.get(i + 1).cloned().unwrap_or_default();
        let mut take = true;
        match k {
            "--id" => {
                obj.insert("id".into(), v.into());
            }
            "--title" | "--subtitle" | "--icon" | "--color" => {
                obj.insert(k[2..].into(), v.into());
            }
            "--final" => {
                obj.insert("final".into(), v.into());
            }
            "--progress" => {
                obj.insert("progress".into(), v.parse::<f64>().unwrap_or(0.0).into());
            }
            "--priority" | "--expires" | "--linger" => {
                let key = if k == "--expires" {
                    "expires_in"
                } else {
                    &k[2..]
                };
                obj.insert(key.into(), v.parse::<i64>().unwrap_or(0).into());
            }
            "--action" => {
                let (id, label) = v.split_once(':').unwrap_or((v.as_str(), v.as_str()));
                actions.push(serde_json::json!({ "id": id, "label": label }));
            }
            "--wait" => {
                wait = true;
                take = false;
            }
            _ => {
                eprintln!("unknown option {k}\n{CLI_HELP}");
                return 2;
            }
        }
        i += if take { 2 } else { 1 };
    }
    if !actions.is_empty() {
        obj.insert("actions".into(), actions.into());
    }
    let line = serde_json::Value::Object(obj).to_string() + "\n";
    let name = wide(PIPE);
    unsafe {
        let h = match CreateFileW(
            PCWSTR(name.as_ptr()),
            (GENERIC_READ | GENERIC_WRITE).0,
            FILE_SHARE_NONE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        ) {
            Ok(h) => h,
            Err(_) => {
                eprintln!("Notchly is not running (or the Live API is turned off).");
                return 1;
            }
        };
        let mut n = 0u32;
        if WriteFile(h, Some(line.as_bytes()), Some(&mut n), None).is_err() {
            eprintln!("write failed");
            let _ = CloseHandle(h);
            return 1;
        }
        if wait {
            let mut buf = [0u8; 4096];
            loop {
                let mut n = 0u32;
                if ReadFile(h, Some(&mut buf), Some(&mut n), None).is_err() || n == 0 {
                    break;
                }
                print!("{}", String::from_utf8_lossy(&buf[..n as usize]));
            }
        } else {
            // give the server a moment to read before the handle closes
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let _ = CloseHandle(h);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_and_validates() {
        let mut m: LiveMsg = serde_json::from_str(r#"{"op":"start","id":"build-42","title":"Build","progress":1.4,"priority":99,"actions":[{"id":"cancel","label":"Cancel"}],"expires_in":600}"#).unwrap();
        m.validate().unwrap();
        assert_eq!(m.progress, Some(1.0));
        assert_eq!(m.priority, Some(85));
        let mut bad: LiveMsg = serde_json::from_str(r#"{"op":"rm","id":"x"}"#).unwrap();
        assert!(bad.validate().is_err());
        let mut noid: LiveMsg = serde_json::from_str(r#"{"op":"end"}"#).unwrap();
        assert!(noid.validate().is_err());
    }
    #[test]
    fn base64() {
        assert_eq!(b64decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(b64decode("aGk").unwrap(), b"hi");
        assert!(b64decode("@@").is_none());
    }
}
