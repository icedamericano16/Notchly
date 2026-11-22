//! Audio visualiser: WASAPI loopback on the default render endpoint → mono →
//! 1024-point FFT → log-spaced bands with attack/release smoothing.
//! Capture runs only while the visual is on screen and music plays.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL};

use super::{com_mta, spawn, Bus, Event};

pub const N: usize = 1024;
pub const BANDS: usize = 32;

/// In-place iterative radix-2 FFT. `re`/`im` length must be a power of two.
pub fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f32::consts::PI / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
        }
        len <<= 1;
    }
}

/// Magnitudes → log-spaced bands (0..1), smoothed with attack/release.
pub fn bands(samples: &[f32], rate: u32, prev: &mut [f32; BANDS]) {
    let mut re: Vec<f32> = samples
        .iter()
        .enumerate()
        .map(|(i, s)| {
            s * (0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (N - 1) as f32).cos())
        })
        .collect();
    let mut im = vec![0.0; N];
    fft(&mut re, &mut im);
    let bin_hz = rate as f32 / N as f32;
    let (lo, hi) = (40.0f32, 16000.0f32.min(rate as f32 / 2.0));
    for (b, out) in prev.iter_mut().enumerate() {
        let f0 = lo * (hi / lo).powf(b as f32 / BANDS as f32);
        let f1 = lo * (hi / lo).powf((b + 1) as f32 / BANDS as f32);
        let (i0, i1) = (
            ((f0 / bin_hz) as usize).max(1),
            ((f1 / bin_hz) as usize).clamp(2, N / 2),
        );
        let mut m = 0.0f32;
        for i in i0..i1.max(i0 + 1) {
            m = m.max((re[i] * re[i] + im[i] * im[i]).sqrt());
        }
        let db = 20.0 * (m / (N as f32 / 4.0) + 1e-6).log10();
        let v = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        let k = if v > *out { 0.6 } else { 0.15 };
        *out += (v - *out) * k;
    }
}

pub fn start(bus: Bus, active: Arc<AtomicBool>) {
    spawn("visualiser", move || {
        com_mta();
        loop {
            if !active.load(Ordering::Relaxed) {
                std::thread::park_timeout(Duration::from_millis(500));
                continue;
            }
            if let Err(e) = unsafe { capture(&bus, &active) } {
                crate::util::log(&format!("visualiser: {e}"));
                std::thread::sleep(Duration::from_secs(3));
            }
        }
    });
}

unsafe fn capture(bus: &Bus, active: &AtomicBool) -> windows::core::Result<()> {
    unsafe {
        let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let dev = en.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let client: IAudioClient = dev.Activate(CLSCTX_ALL, None)?;
        let fmt = client.GetMixFormat()?;
        let channels = (*fmt).nChannels as usize;
        let rate = (*fmt).nSamplesPerSec;
        let bits = (*fmt).wBitsPerSample;
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK,
            200_000,
            0,
            fmt,
            None,
        )?;
        let cap: IAudioCaptureClient = client.GetService()?;
        client.Start()?;
        let mut ring: Vec<f32> = vec![0.0; N];
        let mut out = [0.0f32; BANDS];
        let mut last_send = Instant::now();
        while active.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(15));
            loop {
                let n = cap.GetNextPacketSize()?;
                if n == 0 {
                    break;
                }
                let mut data = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                cap.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
                let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
                for f in 0..frames as usize {
                    let mut s = 0.0f32;
                    if !silent && bits == 32 {
                        let p = data as *const f32;
                        for c in 0..channels {
                            s += *p.add(f * channels + c);
                        }
                    } else if !silent && bits == 16 {
                        let p = data as *const i16;
                        for c in 0..channels {
                            s += *p.add(f * channels + c) as f32 / 32768.0;
                        }
                    }
                    ring.rotate_left(1);
                    ring[N - 1] = s / channels.max(1) as f32;
                }
                cap.ReleaseBuffer(frames)?;
            }
            if last_send.elapsed() >= Duration::from_millis(33) {
                last_send = Instant::now();
                bands(&ring, rate, &mut out);
                bus.send(Event::Viz(out.to_vec()));
            }
        }
        let _ = client.Stop();
        CoTaskMemFree(Some(fmt as _));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fft_finds_tone() {
        let rate = 48000.0;
        let f = 1500.0;
        let mut re: Vec<f32> = (0..N)
            .map(|i| (2.0 * std::f32::consts::PI * f * i as f32 / rate).sin())
            .collect();
        let mut im = vec![0.0; N];
        fft(&mut re, &mut im);
        let peak = (1..N / 2)
            .max_by(|a, b| {
                (re[*a].hypot(im[*a]))
                    .partial_cmp(&re[*b].hypot(im[*b]))
                    .unwrap()
            })
            .unwrap();
        let expected = (f / (rate / N as f32)).round() as usize;
        assert!((peak as i64 - expected as i64).abs() <= 1);
    }
}
