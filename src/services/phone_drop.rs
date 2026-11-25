//! Phone Drop: an on-demand LAN-only web server (random port, QR) for moving files between a
//! phone and the shelf. Auto-stops after 5 min idle.
//!
//! Access: the QR code carries a random one-time token that works once, within two minutes;
//! redeeming it sets a session cookie and drops the token from the address bar. The 6-digit
//! PIN shown in Notchly is only for typing on the login page, as a fallback. Wrong tokens or
//! PINs count towards a limit that stops the session, and new connections are rate-limited.
//! See DECISIONS.md: served over plain HTTP on the LAN, not HTTPS, hence "trusted Wi-Fi only".

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpListener, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::{spawn, Bus, Event};
use crate::util::sub_dir;

#[derive(Debug, Clone, PartialEq)]
pub enum DropEvent {
    Started {
        /// The address to type by hand (with the PIN); the QR holds a one-time link instead.
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

fn random_hex(bytes: usize) -> String {
    crate::platform::credentials::random_bytes(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn start(bus: Bus, send_files: Vec<PathBuf>) -> Option<Server> {
    let Some(ip) = lan_ip() else {
        bus.send(Event::PhoneDrop(DropEvent::Error(
            "Connect to Wi-Fi or a LAN first.".into(),
        )));
        return None;
    };
    // the LAN address only: not reachable through other interfaces (VPN, mobile hotspot)
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
    let token = random_hex(16);
    let base = format!("http://{ip}:{port}/");
    let qr = qr_matrix(&format!("{base}?t={token}")).unwrap_or_default();
    bus.send(Event::PhoneDrop(DropEvent::Started {
        url: base,
        pin: pin.clone(),
        qr,
    }));
    let stop = Arc::new(AtomicBool::new(false));
    let files = Arc::new(Mutex::new(send_files));
    let auth = Arc::new(Mutex::new(Auth::new(token, pin, Instant::now())));
    let (s2, f2) = (stop.clone(), files.clone());
    spawn("phone-drop", move || {
        let _ = listener.set_nonblocking(true);
        let mut last = Instant::now();
        let mut rate = RateLimit::default();
        while !s2.load(Ordering::Relaxed) && last.elapsed() < IDLE_STOP {
            match listener.accept() {
                Ok((stream, peer)) => {
                    if !same_subnet(ip, peer.ip()) || !rate.allow(Instant::now()) {
                        continue; // dropped: the connection closes
                    }
                    last = Instant::now();
                    let _ = stream.set_nonblocking(false);
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
                    let (b, f) = (bus.clone(), f2.clone());
                    let ctx = Ctx {
                        auth: auth.clone(),
                        stop: s2.clone(),
                    };
                    spawn("phone-drop-conn", move || {
                        let _ = handle(stream, &b, &f, &ctx);
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

const STYLE: &str = r#"<meta name="viewport" content="width=device-width,initial-scale=1"><title>Notchly Drop</title>
<style>body{font-family:system-ui;background:#0A0D18;color:#ECEFF7;margin:0;padding:24px}h1{font-size:20px}.card{background:#121726;border-radius:18px;padding:20px;margin:16px 0}
button,label.btn{display:block;width:100%;padding:14px;border:0;border-radius:999px;background:linear-gradient(90deg,#19B8FF,#2D5BFF,#7B4BFF);color:#fff;font-size:16px;text-align:center}
input{box-sizing:border-box;width:100%;padding:14px;border-radius:12px;border:1px solid #2A3350;background:#0A0D18;color:#ECEFF7;font-size:22px;letter-spacing:6px;text-align:center;margin:0 0 12px}
progress{width:100%;height:10px}a{color:#19B8FF}.note{color:#8A93AD;font-size:13px}</style>"#;

const WARNING: &str = "Use Phone Drop only on Wi-Fi you trust: the connection is not encrypted.";

fn main_page(files: &str) -> String {
    format!(
        r#"<!doctype html><html><head>{STYLE}</head><body><h1>Notchly Drop</h1>
<div class="card"><label class="btn">Choose files to send<input id="f" type="file" multiple hidden></label><p id="s"></p><progress id="p" value="0" max="1" hidden></progress></div>
<div class="card"><b>Files from your PC</b><div id="list">{files}</div></div><p class="note">{WARNING}</p>
<script>const f=document.getElementById('f'),s=document.getElementById('s'),p=document.getElementById('p');
f.onchange=async()=>{{for(const file of f.files){{await new Promise(r=>{{const x=new XMLHttpRequest();x.open('POST','/upload?name='+encodeURIComponent(file.name));p.hidden=false;
x.upload.onprogress=e=>{{p.value=e.loaded/e.total;s.textContent='Sending '+file.name}};x.onload=()=>{{s.textContent=x.status==200?'Sent '+file.name:'Failed';r()}};x.onerror=()=>{{s.textContent='Failed';r()}};x.send(file)}})}}}}</script></body></html>"#
    )
}

fn login_page(message: &str) -> String {
    format!(
        r#"<!doctype html><html><head>{STYLE}</head><body><h1>Notchly Drop</h1>
<div class="card"><p>{message}</p><form method="post" action="/login"><input name="pin" inputmode="numeric" autocomplete="one-time-code" maxlength="6" autofocus placeholder="PIN"><button>Open</button></form></div>
<p class="note">{WARNING}</p></body></html>"#
    )
}

/// How long the QR link works if nobody has used it.
const TOKEN_TTL: Duration = Duration::from_secs(120);
/// Wrong tokens or PINs allowed before the session shuts itself down.
const MAX_FAILS: u32 = 8;
/// Phones allowed in one session.
const MAX_SESSIONS: usize = 8;
/// The session ends after this long without a new connection.
const IDLE_STOP: Duration = Duration::from_secs(300);

/// Compares without an early exit, so timing reveals nothing about the secret.
fn same_secret(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[derive(Debug, PartialEq)]
enum Denied {
    /// Wrong or expired; the caller may try again.
    Wrong,
    /// That was one failure too many: the session must stop.
    Stop,
}

/// Who may use this session.
struct Auth {
    /// The QR link's token while it is unused, and when it was issued.
    token: Option<(String, Instant)>,
    pin: String,
    sessions: Vec<String>,
    fails: u32,
}

impl Auth {
    fn new(token: String, pin: String, now: Instant) -> Self {
        Self {
            token: Some((token, now)),
            pin,
            sessions: Vec::new(),
            fails: 0,
        }
    }

    fn open_session(&mut self, id: String) -> Result<String, Denied> {
        if self.sessions.len() >= MAX_SESSIONS {
            return Err(Denied::Wrong);
        }
        self.sessions.push(id.clone());
        Ok(id)
    }

    fn fail(&mut self) -> Denied {
        self.fails += 1;
        if self.fails >= MAX_FAILS {
            Denied::Stop
        } else {
            Denied::Wrong
        }
    }

    /// The QR token: works once, within TOKEN_TTL. `id` becomes the new session's id.
    fn redeem(&mut self, token: &str, id: String, now: Instant) -> Result<String, Denied> {
        let ok = self
            .token
            .as_ref()
            .is_some_and(|(t, at)| now.duration_since(*at) < TOKEN_TTL && same_secret(t, token));
        if !ok {
            return Err(self.fail());
        }
        self.token = None;
        self.open_session(id)
    }

    /// The typed PIN.
    fn login(&mut self, pin: &str, id: String) -> Result<String, Denied> {
        if !same_secret(pin.trim(), &self.pin) {
            return Err(self.fail());
        }
        self.open_session(id)
    }

    fn allowed(&self, session: Option<&str>) -> bool {
        session.is_some_and(|c| self.sessions.iter().any(|s| same_secret(s, c)))
    }
}

/// At most LIMIT new connections in any WINDOW; more are dropped.
#[derive(Default)]
struct RateLimit {
    recent: VecDeque<Instant>,
}

impl RateLimit {
    const LIMIT: usize = 40;
    const WINDOW: Duration = Duration::from_secs(10);

    fn allow(&mut self, now: Instant) -> bool {
        while self
            .recent
            .front()
            .is_some_and(|t| now.duration_since(*t) >= Self::WINDOW)
        {
            self.recent.pop_front();
        }
        if self.recent.len() >= Self::LIMIT {
            return false;
        }
        self.recent.push_back(now);
        true
    }
}

/// What every connection of one session shares.
struct Ctx {
    auth: Arc<Mutex<Auth>>,
    stop: Arc<AtomicBool>,
}

impl Ctx {
    fn auth(&self) -> std::sync::MutexGuard<'_, Auth> {
        self.auth.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn cookie(header_value: &str, name: &str) -> Option<String> {
    header_value
        .split(';')
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_string())
}

fn respond(s: &mut TcpStream, code: &str, ctype: &str, body: &[u8]) -> std::io::Result<()> {
    s.write_all(format!("HTTP/1.1 {code}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n", body.len()).as_bytes())?;
    s.write_all(body)
}

/// To the main page with the session cookie (the redirect also takes the token out of the
/// address bar and history).
fn enter(s: &mut TcpStream, session: &str) -> std::io::Result<()> {
    s.write_all(format!("HTTP/1.1 303 See Other\r\nLocation: /\r\nSet-Cookie: nd={session}; Path=/; HttpOnly; SameSite=Strict\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes())
}

fn denied(
    s: &mut TcpStream,
    bus: &Bus,
    ctx: &Ctx,
    d: Denied,
    message: &str,
) -> std::io::Result<()> {
    if d == Denied::Stop {
        ctx.stop.store(true, Ordering::Relaxed);
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
    respond(
        s,
        "403 Forbidden",
        "text/html; charset=utf-8",
        login_page(message).as_bytes(),
    )
}

fn handle(
    mut s: TcpStream,
    bus: &Bus,
    files: &Arc<Mutex<Vec<PathBuf>>>,
    ctx: &Ctx,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(s.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();
    let mut len: u64 = 0;
    let mut session: Option<String> = None;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h == "\r\n" {
            break;
        }
        let lower = h.to_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            len = v.trim().parse().unwrap_or(0);
        } else if lower.starts_with("cookie:") {
            session = cookie(h[7..].trim(), "nd");
        }
    }
    if ctx.stop.load(Ordering::Relaxed) {
        return respond(&mut s, "410 Gone", "text/plain", b"Phone Drop has stopped.");
    }
    let route = path.split('?').next().unwrap_or("/").to_string();

    // getting in: the QR link, or the typed PIN
    if method == "GET" && route == "/" {
        if let Some(t) = query(&path, "t") {
            let r = ctx.auth().redeem(&t, random_hex(16), Instant::now());
            return match r {
                Ok(id) => enter(&mut s, &id),
                Err(d) => denied(
                    &mut s,
                    bus,
                    ctx,
                    d,
                    "This link was already used or has expired. Type the PIN shown in Notchly.",
                ),
            };
        }
    }
    if method == "POST" && route == "/login" {
        let mut body = vec![0u8; len.min(256) as usize];
        reader.read_exact(&mut body)?;
        let pin = query(&format!("?{}", String::from_utf8_lossy(&body)), "pin").unwrap_or_default();
        let r = ctx.auth().login(&pin, random_hex(16));
        return match r {
            Ok(id) => enter(&mut s, &id),
            Err(d) => denied(&mut s, bus, ctx, d, "Wrong PIN. Try again."),
        };
    }
    if !ctx.auth().allowed(session.as_deref()) {
        return if method == "GET" && route == "/" {
            respond(
                &mut s,
                "200 OK",
                "text/html; charset=utf-8",
                login_page("Type the PIN shown in Notchly.").as_bytes(),
            )
        } else {
            respond(&mut s, "403 Forbidden", "text/plain", b"Not signed in.")
        };
    }

    match (method.as_str(), route.as_str()) {
        ("GET", "/") => {
            let list: String = files
                .lock()
                .map(|f| {
                    f.iter()
                        .enumerate()
                        .map(|(i, p)| {
                            format!(
                                "<p><a href=\"/dl/{i}\">{}</a></p>",
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
                main_page(&list).as_bytes(),
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

    /// Sends one raw HTTP request to `handle` over a loopback socket; returns the response.
    fn exchange(ctx: &Ctx, request: &str) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let req = request.to_string();
        let client = std::thread::spawn(move || {
            let mut c = TcpStream::connect(addr).unwrap();
            c.write_all(req.as_bytes()).unwrap();
            let mut out = String::new();
            let _ = c.read_to_string(&mut out);
            out
        });
        let (server, _) = l.accept().unwrap();
        let (tx, _rx) = std::sync::mpsc::channel();
        let bus = Bus::new(tx, windows::Win32::Foundation::HWND(std::ptr::null_mut()));
        let files = Arc::new(Mutex::new(Vec::new()));
        handle(server, &bus, &files, ctx).unwrap();
        client.join().unwrap()
    }

    fn session_of(resp: &str) -> String {
        resp.lines()
            .find_map(|l| l.strip_prefix("Set-Cookie: nd="))
            .and_then(|v| v.split(';').next())
            .unwrap()
            .to_string()
    }

    #[test]
    fn http_flow_token_cookie_pin() {
        let ctx = Ctx {
            auth: Arc::new(Mutex::new(Auth::new(
                "tok".into(),
                "123456".into(),
                Instant::now(),
            ))),
            stop: Arc::new(AtomicBool::new(false)),
        };
        // the QR link: redirect with a cookie, token gone from the address bar
        let r = exchange(&ctx, "GET /?t=tok HTTP/1.1\r\n\r\n");
        assert!(r.starts_with("HTTP/1.1 303"), "{r}");
        assert!(r.contains("Location: /\r\n") && r.contains("HttpOnly; SameSite=Strict"));
        let sid = session_of(&r);
        let r = exchange(&ctx, &format!("GET / HTTP/1.1\r\nCookie: nd={sid}\r\n\r\n"));
        assert!(
            r.starts_with("HTTP/1.1 200") && r.contains("Files from your PC"),
            "{r}"
        );
        // the same link again: refused, PIN form instead
        let r = exchange(&ctx, "GET /?t=tok HTTP/1.1\r\n\r\n");
        assert!(
            r.starts_with("HTTP/1.1 403") && r.contains("already used"),
            "{r}"
        );
        // no cookie: login page, and nothing else works
        let r = exchange(&ctx, "GET / HTTP/1.1\r\n\r\n");
        assert!(r.contains("action=\"/login\"") && !r.contains("Files from your PC"));
        let r = exchange(&ctx, "GET /dl/0 HTTP/1.1\r\n\r\n");
        assert!(r.starts_with("HTTP/1.1 403"));
        // the typed PIN
        let r = exchange(
            &ctx,
            "POST /login HTTP/1.1\r\nContent-Length: 10\r\n\r\npin=000000",
        );
        assert!(r.starts_with("HTTP/1.1 403"));
        let r = exchange(
            &ctx,
            "POST /login HTTP/1.1\r\nContent-Length: 10\r\n\r\npin=123456",
        );
        assert!(r.starts_with("HTTP/1.1 303"), "{r}");
        assert_ne!(session_of(&r), sid);
    }

    #[test]
    fn qr_token_works_once_and_expires() {
        let t0 = Instant::now();
        let mut a = Auth::new("tok".into(), "123456".into(), t0);
        assert_eq!(a.redeem("tok", "s1".into(), t0), Ok("s1".into()));
        assert!(a.allowed(Some("s1")));
        // a second scan (or a copied link) is refused
        assert_eq!(a.redeem("tok", "s2".into(), t0), Err(Denied::Wrong));
        assert!(!a.allowed(Some("s2")));
        // an unused link expires
        let mut b = Auth::new("tok".into(), "123456".into(), t0);
        assert_eq!(
            b.redeem("tok", "s".into(), t0 + TOKEN_TTL),
            Err(Denied::Wrong)
        );
        assert!(!b.allowed(None));
        assert!(!b.allowed(Some("")));
    }

    #[test]
    fn pin_fallback_and_failure_limit() {
        let t0 = Instant::now();
        let mut a = Auth::new("tok".into(), "123456".into(), t0);
        assert_eq!(a.login(" 123456 ", "p1".into()), Ok("p1".into()));
        for _ in 1..MAX_FAILS {
            assert_eq!(a.login("000000", "x".into()), Err(Denied::Wrong));
        }
        // wrong tokens count too; the limit stops the session
        assert_eq!(a.redeem("nope", "x".into(), t0), Err(Denied::Stop));
        assert!(a.allowed(Some("p1")));
    }

    #[test]
    fn rate_limit_window() {
        let t0 = Instant::now();
        let mut r = RateLimit::default();
        for _ in 0..RateLimit::LIMIT {
            assert!(r.allow(t0));
        }
        assert!(!r.allow(t0 + Duration::from_secs(1)));
        assert!(r.allow(t0 + RateLimit::WINDOW));
    }

    #[test]
    fn cookies_and_pages() {
        assert_eq!(cookie("a=1; nd=abc; b=2", "nd").as_deref(), Some("abc"));
        assert_eq!(cookie("a=1", "nd"), None);
        // no PIN or token ends up in links or scripts of the main page
        let page = main_page("<p><a href=\"/dl/0\">x</a></p>");
        assert!(!page.contains("pin"));
        assert!(page.contains(WARNING));
        assert!(login_page("hi").contains("action=\"/login\""));
    }

    #[test]
    fn pin_compare() {
        assert!(same_secret("123456", "123456"));
        assert!(!same_secret("123456", "123457"));
        assert!(!same_secret("12345", "123456"));
        assert!(!same_secret("", "123456"));
    }
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
            qr_matrix("http://192.168.1.2:5000/?t=0123456789abcdef0123456789abcdef")
                .unwrap()
                .len()
                >= 21
        );
    }
}
