//! Phone Drop: an on-demand LAN-only web server (random port, 6-digit PIN, QR)
//! for moving files between a phone and the shelf. Auto-stops after 5 min idle.
//! See DECISIONS.md: served over plain HTTP on the LAN (PIN-protected), not HTTPS.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpListener, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::{spawn, Bus, Event};
use crate::util::sub_dir;

#[derive(Debug, Clone, PartialEq)]
pub enum DropEvent {
    Started {
        url: String,
        pin: String,
        qr: Vec<Vec<bool>>,
    },
    Progress {
        name: String,
        frac: f32,
    },
    Received {
        path: String,
    },
    Stopped,
    Error(String),
}

pub struct Server {
    stop: Arc<AtomicBool>,
    pub send_files: Arc<Mutex<Vec<PathBuf>>>,
}

impl Server {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn lan_ip() -> Option<Ipv4Addr> {
    // connecting a UDP socket sends nothing; it only selects the outgoing interface
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V4(v) if v.is_private() => Some(v),
        _ => None,
    }
}

fn same_subnet(a: Ipv4Addr, b: IpAddr) -> bool {
    match b {
        IpAddr::V4(b) => b.is_private() && a.octets()[..3] == b.octets()[..3] || b.is_loopback(),
        _ => false,
    }
}

pub fn qr_matrix(text: &str) -> Option<Vec<Vec<bool>>> {
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    let w = code.width();
    let colors = code.to_colors();
    Some(
        (0..w)
            .map(|y| {
                (0..w)
                    .map(|x| colors[y * w + x] == qrcode::Color::Dark)
                    .collect()
            })
            .collect(),
    )
}

pub fn start(bus: Bus, send_files: Vec<PathBuf>) -> Option<Server> {
    let Some(ip) = lan_ip() else {
        bus.send(Event::PhoneDrop(DropEvent::Error(
            "Connect to Wi-Fi or a LAN first.".into(),
        )));
        return None;
    };
    let listener = TcpListener::bind((ip, 0)).ok()?;
    let port = listener.local_addr().ok()?.port();
    let pin = format!(
        "{:06}",
        u32::from_le_bytes(
            crate::platform::credentials::random_bytes(4)
                .try_into()
                .unwrap_or([7, 3, 1, 9])
        ) % 1_000_000
    );
    let url = format!("http://{ip}:{port}/?pin={pin}");
    let qr = qr_matrix(&url).unwrap_or_default();
    bus.send(Event::PhoneDrop(DropEvent::Started {
        url,
        pin: pin.clone(),
        qr,
    }));
    let stop = Arc::new(AtomicBool::new(false));
    let files = Arc::new(Mutex::new(send_files));
    let fails = Arc::new(AtomicU32::new(0));
    let (s2, f2) = (stop.clone(), files.clone());
    spawn("phone-drop", move || {
        let _ = listener.set_nonblocking(true);
        let mut last = Instant::now();
        while !s2.load(Ordering::Relaxed) && last.elapsed() < Duration::from_secs(300) {
            match listener.accept() {
                Ok((stream, peer)) => {
                    if !same_subnet(ip, peer.ip()) {
                        continue;
                    }
                    last = Instant::now();
                    let _ = stream.set_nonblocking(false);
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
                    let (b, p, f) = (bus.clone(), pin.clone(), f2.clone());
                    let guard = Guard {
                        fails: fails.clone(),
                        stop: s2.clone(),
                    };
                    spawn("phone-drop-conn", move || {
                        let _ = handle(stream, &b, &p, &f, &guard);
                    });
                }
                Err(_) => std::thread::sleep(Duration::from_millis(120)),
            }
        }
        bus.send(Event::PhoneDrop(DropEvent::Stopped));
    });
    Some(Server {
        stop,
        send_files: files,
    })
}

fn query(path: &str, key: &str) -> Option<String> {
    let q = path.split_once('?')?.1;
    q.split('&')
        .find_map(|kv| kv.strip_prefix(&format!("{key}=")))
        .map(url_decode)
}

fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
                out.push(b'%');
            }
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn safe_name(n: &str) -> String {
    let n: String = n
        .chars()
        .filter(|c| {
            !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control()
        })
        .collect();
    let n = n.trim().trim_start_matches('.').to_string();
    if n.is_empty() {
        "file".into()
    } else {
        n.chars().take(120).collect()
    }
}

fn respond(s: &mut TcpStream, code: &str, ctype: &str, body: &[u8]) -> std::io::Result<()> {
    s.write_all(format!("HTTP/1.1 {code}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", body.len()).as_bytes())?;
    s.write_all(body)
}

const PAGE: &str = r#"<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1"><title>Notchly Drop</title>
<style>body{font-family:system-ui;background:#0A0D18;color:#ECEFF7;margin:0;padding:24px}h1{font-size:20px}.card{background:#121726;border-radius:18px;padding:20px;margin:16px 0}
button,label.btn{display:block;width:100%;padding:14px;border:0;border-radius:999px;background:linear-gradient(90deg,#19B8FF,#2D5BFF,#7B4BFF);color:#fff;font-size:16px;text-align:center}
progress{width:100%;height:10px}a{color:#19B8FF}</style></head><body><h1>Notchly Drop</h1>
<div class="card"><label class="btn">Choose files to send<input id="f" type="file" multiple hidden></label><p id="s"></p><progress id="p" value="0" max="1" hidden></progress></div>
<div class="card"><b>Files from your PC</b><div id="list">FILES</div></div>
<script>const pin=new URLSearchParams(location.search).get('pin');const f=document.getElementById('f'),s=document.getElementById('s'),p=document.getElementById('p');
f.onchange=async()=>{for(const file of f.files){await new Promise(r=>{const x=new XMLHttpRequest();x.open('POST','/upload?pin='+pin+'&name='+encodeURIComponent(file.name));p.hidden=false;
x.upload.onprogress=e=>{p.value=e.loaded/e.total;s.textContent='Sending '+file.name};x.onload=()=>{s.textContent='Sent '+file.name;r()};x.onerror=()=>{s.textContent='Failed';r()};x.send(file)})}}</script></body></html>"#;

/// Wrong-PIN protection shared by all connections of one session.
struct Guard {
    fails: Arc<AtomicU32>,
    stop: Arc<AtomicBool>,
}

/// Wrong PINs allowed before the session shuts itself down.
const MAX_PIN_FAILS: u32 = 8;

/// Compares without an early exit, so timing reveals nothing about the PIN.
fn same_secret(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn handle(
    mut s: TcpStream,
    bus: &Bus,
    pin: &str,
    files: &Arc<Mutex<Vec<PathBuf>>>,
    guard: &Guard,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(s.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();
    let mut len: u64 = 0;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h == "\r\n" {
            break;
        }
        if let Some(v) = h.to_lowercase().strip_prefix("content-length:") {
            len = v.trim().parse().unwrap_or(0);
        }
    }
    if guard.stop.load(Ordering::Relaxed) {
        return respond(&mut s, "410 Gone", "text/plain", b"Phone Drop has stopped.");
    }
    if !query(&path, "pin").is_some_and(|p| same_secret(&p, pin)) {
        let n = guard.fails.fetch_add(1, Ordering::Relaxed) + 1;
        if n >= MAX_PIN_FAILS {
            guard.stop.store(true, Ordering::Relaxed);
            bus.send(Event::PhoneDrop(DropEvent::Error(
                "Stopped after repeated wrong PINs".into(),
            )));
            bus.send(Event::Notice(
                false,
                "Phone Drop stopped".into(),
                "Someone kept trying wrong PINs".into(),
            ));
        }
        // slow down guessing
        std::thread::sleep(Duration::from_millis(600));
        return respond(
            &mut s,
            "403 Forbidden",
            "text/plain",
            b"Wrong or missing PIN. Scan the QR code shown in Notchly.",
        );
    }
    let route = path.split('?').next().unwrap_or("/");
    match (method.as_str(), route) {
        ("GET", "/") => {
            let list: String = files
                .lock()
                .map(|f| {
                    f.iter()
                        .enumerate()
                        .map(|(i, p)| {
                            format!(
                                "<p><a href=\"/dl/{i}?pin={pin}\">{}</a></p>",
                                html_escape(
                                    &p.file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_default()
                                )
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            let list = if list.is_empty() {
                "<p>Nothing shared yet.</p>".to_string()
            } else {
                list
            };
            respond(
                &mut s,
                "200 OK",
                "text/html; charset=utf-8",
                PAGE.replace("FILES", &list).as_bytes(),
            )
        }
        ("POST", "/upload") => {
            let name = safe_name(&query(&path, "name").unwrap_or_default());
            let dir = sub_dir("shelf/received");
            let mut target = dir.join(&name);
            let mut k = 1;
            while target.exists() {
                target = dir.join(format!("{k} {name}"));
                k += 1;
            }
            let mut out = std::fs::File::create(&target)?;
            let mut left = len;
            let mut buf = vec![0u8; 256 * 1024];
            let mut last = Instant::now();
            while left > 0 {
                let cap = left.min(buf.len() as u64) as usize;
                let n = reader.read(&mut buf[..cap])?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n])?;
                left -= n as u64;
                if last.elapsed() > Duration::from_millis(250) {
                    last = Instant::now();
                    bus.send(Event::PhoneDrop(DropEvent::Progress {
                        name: name.clone(),
                        frac: 1.0 - left as f32 / len.max(1) as f32,
                    }));
                }
            }
            drop(out);
            if left == 0 {
                bus.send(Event::PhoneDrop(DropEvent::Received {
                    path: target.to_string_lossy().to_string(),
                }));
                respond(&mut s, "200 OK", "text/plain", b"ok")
            } else {
                let _ = std::fs::remove_file(&target);
                respond(&mut s, "400 Bad Request", "text/plain", b"incomplete")
            }
        }
        ("GET", r) if r.starts_with("/dl/") => {
            let idx: usize = r[4..].parse().unwrap_or(usize::MAX);
            let p = files.lock().ok().and_then(|f| f.get(idx).cloned());
            match p.and_then(|p| std::fs::File::open(&p).ok().map(|f| (p, f))) {
                Some((p, mut f)) => {
                    let size = f.metadata()?.len();
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {size}\r\nContent-Disposition: attachment; filename=\"{}\"\r\nConnection: close\r\n\r\n", safe_name(&name)).as_bytes())?;
                    std::io::copy(&mut f, &mut s)?;
                    Ok(())
                }
                None => respond(&mut s, "404 Not Found", "text/plain", b"not found"),
            }
        }
        _ => respond(&mut s, "404 Not Found", "text/plain", b"not found"),
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_are_sanitised() {
        assert_eq!(safe_name("../../evil.exe"), "evil.exe");
        assert_eq!(safe_name("a<b>.txt"), "ab.txt");
        assert_eq!(url_decode("hello%20world+x"), "hello world x");
    }
    #[test]
    fn subnet_rules() {
        let me = Ipv4Addr::new(192, 168, 1, 10);
        assert!(same_subnet(me, "192.168.1.55".parse().unwrap()));
        assert!(!same_subnet(me, "192.168.2.55".parse().unwrap()));
        assert!(!same_subnet(me, "8.8.8.8".parse().unwrap()));
    }
    #[test]
    fn qr_builds() {
        assert!(
            qr_matrix("http://192.168.1.2:5000/?pin=123456")
                .unwrap()
                .len()
                >= 21
        );
    }
}
