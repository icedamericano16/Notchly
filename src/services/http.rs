//! HTTP via WinRT `Windows.Web.Http.HttpClient` (no TLS crate in the binary).
//! Blocking helpers for worker threads: 10 s timeout, identifying User-Agent.

use std::time::{Duration, Instant};

use windows::core::HSTRING;
use windows::Foundation::Uri;
use windows::Storage::Streams::{DataReader, InputStreamOptions, UnicodeEncoding};
use windows::Web::Http::{
    HttpClient, HttpCompletionOption, HttpMethod, HttpRequestMessage, HttpStringContent,
};
use windows_future::AsyncStatus;

pub const TIMEOUT: Duration = Duration::from_secs(10);

pub fn user_agent() -> String {
    format!(
        "Notchly/{} (+https://notchly.app)",
        env!("CARGO_PKG_VERSION")
    )
}

#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub body: String,
    pub headers: Vec<(String, String)>,
}

impl Response {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Waits for an async operation with a timeout, cancelling it on expiry.
macro_rules! wait {
    ($op:expr, $timeout:expr) => {{
        let op = $op;
        let start = Instant::now();
        loop {
            match op.Status().map_err(|e| e.to_string())? {
                AsyncStatus::Started => {
                    if start.elapsed() > $timeout {
                        let _ = op.Cancel();
                        return Err("timed out".to_string());
                    }
                    std::thread::sleep(Duration::from_millis(8));
                }
                _ => break op.GetResults().map_err(|e| e.message().to_string()),
            }
        }
    }};
}

fn request(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<(&str, &str)>,
) -> Result<HttpRequestMessage, String> {
    let m = match method {
        "POST" => HttpMethod::Post(),
        "PUT" => HttpMethod::Put(),
        "DELETE" => HttpMethod::Delete(),
        _ => HttpMethod::Get(),
    }
    .map_err(|e| e.to_string())?;
    let uri = Uri::CreateUri(&HSTRING::from(url)).map_err(|e| e.to_string())?;
    let req = HttpRequestMessage::Create(&m, &uri).map_err(|e| e.to_string())?;
    let h = req.Headers().map_err(|e| e.to_string())?;
    let _ =
        h.TryAppendWithoutValidation(&HSTRING::from("User-Agent"), &HSTRING::from(user_agent()));
    for (k, v) in headers {
        let _ = h.TryAppendWithoutValidation(&HSTRING::from(*k), &HSTRING::from(*v));
    }
    if let Some((ctype, data)) = body {
        let content = HttpStringContent::CreateFromStringWithEncodingAndMediaType(
            &HSTRING::from(data),
            UnicodeEncoding::Utf8,
            &HSTRING::from(ctype),
        )
        .map_err(|e| e.to_string())?;
        req.SetContent(&content).map_err(|e| e.to_string())?;
    }
    Ok(req)
}

fn client() -> Result<HttpClient, String> {
    HttpClient::new().map_err(|e| e.to_string())
}

/// Performs a request and reads the whole body as text.
pub fn send(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<(&str, &str)>,
) -> Result<Response, String> {
    if !url.starts_with("https://")
        && !url.starts_with("http://127.0.0.1")
        && !url.starts_with("http://localhost")
    {
        return Err("only HTTPS is allowed".into());
    }
    let c = client()?;
    let req = request(method, url, headers, body)?;
    let resp = wait!(
        c.SendRequestAsync(&req).map_err(|e| e.to_string())?,
        TIMEOUT
    )?;
    let status = resp.StatusCode().map_err(|e| e.to_string())?.0 as u16;
    let mut hdrs = Vec::new();
    if let Ok(h) = resp.Headers() {
        if let Ok(it) = h.First() {
            while it.HasCurrent().unwrap_or(false) {
                if let Ok(kv) = it.Current() {
                    hdrs.push((
                        kv.Key().map(|s| s.to_string()).unwrap_or_default(),
                        kv.Value().map(|s| s.to_string()).unwrap_or_default(),
                    ));
                }
                if !it.MoveNext().unwrap_or(false) {
                    break;
                }
            }
        }
    }
    let content = resp.Content().map_err(|e| e.to_string())?;
    let body = wait!(
        content.ReadAsStringAsync().map_err(|e| e.to_string())?,
        TIMEOUT
    )?
    .to_string();
    Ok(Response {
        status,
        body,
        headers: hdrs,
    })
}

pub fn get(url: &str, headers: &[(&str, &str)]) -> Result<Response, String> {
    send("GET", url, headers, None)
}

pub fn post_form(url: &str, form: &str) -> Result<Response, String> {
    send(
        "POST",
        url,
        &[],
        Some(("application/x-www-form-urlencoded", form)),
    )
}

/// Streams a response body line by line. `on_line` returns false to stop.
/// `cancel` is polled between chunks.
pub fn stream_lines(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<(&str, &str)>,
    cancel: &dyn Fn() -> bool,
    on_line: &mut dyn FnMut(&str) -> bool,
) -> Result<u16, String> {
    let c = client()?;
    let req = request(method, url, headers, body)?;
    let resp = wait!(
        c.SendRequestWithOptionAsync(&req, HttpCompletionOption::ResponseHeadersRead)
            .map_err(|e| e.to_string())?,
        TIMEOUT
    )?;
    let status = resp.StatusCode().map_err(|e| e.to_string())?.0 as u16;
    let content = resp.Content().map_err(|e| e.to_string())?;
    if !(200..300).contains(&status) {
        let text = wait!(
            content.ReadAsStringAsync().map_err(|e| e.to_string())?,
            TIMEOUT
        )?
        .to_string();
        return Err(format!(
            "HTTP {status}: {}",
            text.chars().take(300).collect::<String>()
        ));
    }
    let stream = wait!(
        content
            .ReadAsInputStreamAsync()
            .map_err(|e| e.to_string())?,
        TIMEOUT
    )?;
    let reader = DataReader::CreateDataReader(&stream).map_err(|e| e.to_string())?;
    reader
        .SetInputStreamOptions(InputStreamOptions::Partial)
        .map_err(|e| e.to_string())?;
    let mut pending: Vec<u8> = Vec::new();
    loop {
        if cancel() {
            return Ok(status);
        }
        let n = wait!(
            reader.LoadAsync(16 * 1024).map_err(|e| e.to_string())?,
            Duration::from_secs(60)
        )?;
        if n == 0 {
            break;
        }
        let mut buf = vec![0u8; n as usize];
        reader.ReadBytes(&mut buf).map_err(|e| e.to_string())?;
        pending.extend_from_slice(&buf);
        while let Some(pos) = pending.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = pending.drain(..=pos).collect();
            let s = String::from_utf8_lossy(&line);
            let s = s.trim_end_matches(['\r', '\n']);
            if !s.is_empty() && !on_line(s) {
                return Ok(status);
            }
        }
    }
    if !pending.is_empty() {
        let s = String::from_utf8_lossy(&pending).to_string();
        on_line(s.trim());
    }
    Ok(status)
}

/// Percent-encodes a query component.
pub fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Exponential backoff tracker for a network feature.
#[derive(Debug, Clone, Default)]
pub struct Backoff {
    failures: u32,
    until: Option<Instant>,
}

impl Backoff {
    pub fn ready(&self) -> bool {
        self.until.map(|u| Instant::now() >= u).unwrap_or(true)
    }
    pub fn fail(&mut self) {
        self.failures = (self.failures + 1).min(8);
        self.until = Some(Instant::now() + Duration::from_secs(15 * (1u64 << self.failures)));
    }
    pub fn ok(&mut self) {
        self.failures = 0;
        self.until = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encodes() {
        assert_eq!(enc("a b&c"), "a%20b%26c");
        assert!(enc("नमस्ते").len() > 6);
    }
}
