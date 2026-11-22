//! Smart OCR with Windows.Media.Ocr (fully offline) and quick-action detection.

use std::sync::Arc;

use windows::core::HSTRING;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Security::Cryptography::CryptographicBuffer;

use super::{spawn, Bus, Event};
use crate::render::image::Pixels;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct OcrResult {
    pub text: String,
    pub words: usize,
    pub urls: Vec<String>,
    pub emails: Vec<String>,
    pub phones: Vec<String>,
    /// Decoded QR code contents (decoded locally).
    pub qr: Vec<String>,
}

pub fn available_languages() -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Ok(list) = OcrEngine::AvailableRecognizerLanguages() {
        for i in 0..list.Size().unwrap_or(0) {
            if let Ok(l) = list.GetAt(i) {
                out.push((
                    l.LanguageTag().map(|s| s.to_string()).unwrap_or_default(),
                    l.DisplayName().map(|s| s.to_string()).unwrap_or_default(),
                ));
            }
        }
    }
    out
}

/// Recognises text in BGRA pixels on a worker.
pub fn recognize(bus: Bus, px: Arc<Pixels>, lang: String) {
    spawn("ocr", move || {
        super::com_mta();
        bus.send(Event::Ocr(run(&px, &lang)));
    });
}

/// OCR + local QR decoding. A QR code alone (no OCR language installed) still succeeds.
fn run(px: &Pixels, lang: &str) -> Result<OcrResult, String> {
    let qr = decode_qr(px);
    let text = match ocr_text(px, lang) {
        Ok(t) => t,
        Err(_) if !qr.is_empty() => String::new(),
        Err(e) => return Err(e),
    };
    let text = if text.trim().is_empty() {
        qr.join("\n")
    } else {
        text
    };
    let mut res = analyse(&text);
    for q in &qr {
        if (q.starts_with("http://") || q.starts_with("https://")) && !res.urls.contains(q) {
            res.urls.insert(0, q.clone());
        }
    }
    res.qr = qr;
    Ok(res)
}

/// Finds and decodes QR codes in BGRA pixels.
pub fn decode_qr(px: &Pixels) -> Vec<String> {
    if px.w < 21 || px.h < 21 || px.w > 6000 || px.h > 6000 {
        return Vec::new();
    }
    let (w, h) = (px.w as usize, px.h as usize);
    let d = &px.data;
    let mut img = rqrr::PreparedImage::prepare_from_greyscale(w, h, |x, y| {
        let o = (y * w + x) * 4;
        ((d[o] as u32 * 29 + d[o + 1] as u32 * 150 + d[o + 2] as u32 * 77) >> 8) as u8
    });
    img.detect_grids()
        .into_iter()
        .filter_map(|g| g.decode().ok().map(|(_, s)| s))
        .collect()
}

fn ocr_text(px: &Pixels, lang: &str) -> Result<String, String> {
    let max = OcrEngine::MaxImageDimension().unwrap_or(10000);
    if px.w > max || px.h > max {
        return Err("Selection too large for OCR".into());
    }
    let buf = CryptographicBuffer::CreateFromByteArray(&px.data).map_err(|e| e.to_string())?;
    let bmp = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
        &buf,
        BitmapPixelFormat::Bgra8,
        px.w as i32,
        px.h as i32,
        BitmapAlphaMode::Premultiplied,
    )
    .map_err(|e| e.to_string())?;
    let engine = if lang.is_empty() {
        OcrEngine::TryCreateFromUserProfileLanguages()
    } else {
        Language::CreateLanguage(&HSTRING::from(lang))
            .and_then(|l| OcrEngine::TryCreateFromLanguage(&l))
    }
    .map_err(|_| {
        "No OCR language installed. Add one in Settings › Time & language › Language.".to_string()
    })?;
    let res = engine
        .RecognizeAsync(&bmp)
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|e| e.to_string())?;
    let mut lines = Vec::new();
    if let Ok(ls) = res.Lines() {
        for i in 0..ls.Size().unwrap_or(0) {
            if let Ok(l) = ls.GetAt(i) {
                lines.push(l.Text().map(|s| s.to_string()).unwrap_or_default());
            }
        }
    }
    Ok(lines.join("\n"))
}

/// Extracts URLs, e-mails and phone numbers.
pub fn analyse(text: &str) -> OcrResult {
    let mut r = OcrResult {
        text: text.to_string(),
        words: text.split_whitespace().count(),
        ..Default::default()
    };
    for raw in text.split_whitespace() {
        let w =
            raw.trim_matches(|c: char| matches!(c, ',' | ';' | ')' | '(' | '"' | '\'' | '<' | '>'));
        let wl = w.to_lowercase();
        if wl.starts_with("http://")
            || wl.starts_with("https://")
            || (wl.starts_with("www.") && wl.len() > 6)
        {
            r.urls.push(if wl.starts_with("www.") {
                format!("https://{w}")
            } else {
                w.trim_end_matches('.').to_string()
            });
        } else if let Some((a, b)) = w.split_once('@') {
            if !a.is_empty()
                && b.contains('.')
                && !b.ends_with('.')
                && b.chars()
                    .all(|c| c.is_alphanumeric() || c == '.' || c == '-')
            {
                r.emails.push(w.trim_end_matches('.').to_string());
            }
        }
    }
    // phone numbers: runs of digits/spaces/dashes with 7–15 digits
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        let digits = cur.chars().filter(|c| c.is_ascii_digit()).count();
        if (7..=15).contains(&digits) {
            out.push(cur.trim().trim_end_matches(['-', ' ']).to_string());
        }
        cur.clear();
    };
    for c in text.chars() {
        if c.is_ascii_digit()
            || (c == '+' && cur.is_empty())
            || ((c == ' ' || c == '-' || c == '(' || c == ')') && !cur.is_empty())
        {
            cur.push(c);
        } else {
            flush(&mut cur, &mut r.phones);
        }
    }
    flush(&mut cur, &mut r.phones);
    r.urls.dedup();
    r.emails.dedup();
    r.phones.dedup();
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_actions() {
        let r = analyse("Visit https://notchly.app or www.example.com, mail hi@notchly.app\nCall +977 98-1234-5678 today");
        assert_eq!(r.urls.len(), 2);
        assert_eq!(r.emails, vec!["hi@notchly.app".to_string()]);
        assert_eq!(r.phones.len(), 1);
        assert!(r.words > 5);
    }
}
