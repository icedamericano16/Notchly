//! System monitor via PDH: CPU, RAM, GPU engine, disk and network throughput.
//! Samples at 1 Hz only while visible (or every 2 s when the high-CPU alert is on).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use windows::core::PCWSTR;
use windows::Win32::System::Performance::*;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

use super::{spawn, Bus, Event};
use crate::util::wide;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Sample {
    pub cpu: f32,
    pub ram: f32,
    pub gpu: f32,
    pub disk_bps: f64,
    pub net_bps: f64,
}

struct Counters {
    q: PDH_HQUERY,
    cpu: PDH_HCOUNTER,
    gpu: Option<PDH_HCOUNTER>,
    disk: Option<PDH_HCOUNTER>,
    net: Option<PDH_HCOUNTER>,
}

unsafe fn add(q: PDH_HQUERY, path: &str) -> Option<PDH_HCOUNTER> {
    let w = wide(path);
    let mut c = PDH_HCOUNTER::default();
    (unsafe { PdhAddEnglishCounterW(q, PCWSTR(w.as_ptr()), 0, &mut c) } == 0).then_some(c)
}

unsafe fn single(c: PDH_HCOUNTER) -> f64 {
    let mut v = PDH_FMT_COUNTERVALUE::default();
    if unsafe { PdhGetFormattedCounterValue(c, PDH_FMT_DOUBLE, None, &mut v) } == 0 {
        unsafe { v.Anonymous.doubleValue }
    } else {
        0.0
    }
}

/// Sums every instance of a wildcard counter.
unsafe fn sum(c: PDH_HCOUNTER) -> f64 {
    let mut size = 0u32;
    let mut count = 0u32;
    unsafe {
        let _ = PdhGetFormattedCounterArrayW(c, PDH_FMT_DOUBLE, &mut size, &mut count, None);
        if size == 0 {
            return 0.0;
        }
        let mut buf = vec![0u8; size as usize];
        if PdhGetFormattedCounterArrayW(
            c,
            PDH_FMT_DOUBLE,
            &mut size,
            &mut count,
            Some(buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W),
        ) != 0
        {
            return 0.0;
        }
        let items = std::slice::from_raw_parts(
            buf.as_ptr() as *const PDH_FMT_COUNTERVALUE_ITEM_W,
            count as usize,
        );
        items.iter().map(|i| i.FmtValue.Anonymous.doubleValue).sum()
    }
}

fn ram_percent() -> f32 {
    let mut m = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    unsafe {
        let _ = GlobalMemoryStatusEx(&mut m);
    }
    m.dwMemoryLoad as f32
}

pub fn start(bus: Bus, active: Arc<AtomicBool>, alert: Arc<AtomicBool>) {
    spawn("sysmon", move || unsafe {
        let mut counters: Option<Counters> = None;
        loop {
            let on = active.load(Ordering::Relaxed);
            let watch = alert.load(Ordering::Relaxed);
            if !on && !watch {
                // drop the query while hidden (0 % CPU)
                if let Some(c) = counters.take() {
                    let _ = PdhCloseQuery(c.q);
                }
                std::thread::park_timeout(Duration::from_secs(5));
                continue;
            }
            if counters.is_none() {
                let mut q = PDH_HQUERY::default();
                if PdhOpenQueryW(PCWSTR::null(), 0, &mut q) != 0 {
                    std::thread::sleep(Duration::from_secs(10));
                    continue;
                }
                let Some(cpu) = add(q, r"\Processor(_Total)\% Processor Time") else {
                    let _ = PdhCloseQuery(q);
                    std::thread::sleep(Duration::from_secs(10));
                    continue;
                };
                counters = Some(Counters {
                    q,
                    cpu,
                    gpu: add(q, r"\GPU Engine(*engtype_3D)\Utilization Percentage"),
                    disk: add(q, r"\PhysicalDisk(_Total)\Disk Bytes/sec"),
                    net: add(q, r"\Network Interface(*)\Bytes Total/sec"),
                });
                let _ = PdhCollectQueryData(q);
                std::thread::sleep(Duration::from_millis(500));
            }
            if let Some(c) = &counters {
                let _ = PdhCollectQueryData(c.q);
                let s = Sample {
                    cpu: single(c.cpu) as f32,
                    ram: ram_percent(),
                    gpu: c.gpu.map(|g| sum(g).min(100.0) as f32).unwrap_or(0.0),
                    disk_bps: c.disk.map(|d| single(d)).unwrap_or(0.0),
                    net_bps: c.net.map(|n| sum(n)).unwrap_or(0.0),
                };
                bus.send(Event::SysMon(s));
            }
            std::thread::sleep(Duration::from_millis(if on { 1000 } else { 2000 }));
        }
    });
}

/// Process using the most CPU over a short window (name only).
pub fn top_process() -> Option<String> {
    use windows::Win32::Foundation::{CloseHandle, FILETIME};
    use windows::Win32::System::Diagnostics::ToolHelp::*;
    use windows::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe fn times() -> Vec<(u32, String, u64)> {
        let mut out = Vec::new();
        let Ok(snap) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
            return out;
        };
        let mut e = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut ok = unsafe { Process32FirstW(snap, &mut e) }.is_ok();
        while ok {
            let name = crate::util::from_wide(&e.szExeFile);
            if let Ok(h) =
                unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, e.th32ProcessID) }
            {
                let (mut a, mut b, mut k, mut u) = (
                    FILETIME::default(),
                    FILETIME::default(),
                    FILETIME::default(),
                    FILETIME::default(),
                );
                if unsafe { GetProcessTimes(h, &mut a, &mut b, &mut k, &mut u) }.is_ok() {
                    let t = ((k.dwHighDateTime as u64) << 32 | k.dwLowDateTime as u64)
                        + ((u.dwHighDateTime as u64) << 32 | u.dwLowDateTime as u64);
                    out.push((e.th32ProcessID, name, t));
                }
                let _ = unsafe { CloseHandle(h) };
            }
            ok = unsafe { Process32NextW(snap, &mut e) }.is_ok();
        }
        let _ = unsafe { CloseHandle(snap) };
        out
    }
    unsafe {
        let a = times();
        std::thread::sleep(Duration::from_millis(400));
        let b = times();
        b.iter()
            .filter(|(pid, _, _)| *pid != 0)
            .filter_map(|(pid, name, t)| {
                a.iter()
                    .find(|x| x.0 == *pid)
                    .map(|x| (name.clone(), t.saturating_sub(x.2)))
            })
            .max_by_key(|x| x.1)
            .map(|x| x.0)
    }
}

pub fn fmt_rate(bps: f64) -> String {
    if bps >= 1e9 {
        format!("{:.1} GB/s", bps / 1e9)
    } else if bps >= 1e6 {
        format!("{:.1} MB/s", bps / 1e6)
    } else if bps >= 1e3 {
        format!("{:.0} KB/s", bps / 1e3)
    } else {
        format!("{bps:.0} B/s")
    }
}
