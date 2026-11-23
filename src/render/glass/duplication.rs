//! The pixels behind the notch, for the Liquid tier.
//!
//! A worker thread owns a DXGI Desktop Duplication session on the notch's display and copies
//! only the canvas-sized region under the notch into a GPU texture shared with the render
//! device (keyed mutex), and only when the screen actually changed there (dirty / move
//! rects). The notch window itself is excluded from capture (`WDA_EXCLUDEFROMCAPTURE`), so it
//! never sees itself. Nothing is ever read back to the CPU, logged or written anywhere.
//!
//! The render thread opens the shared texture, copies new frames into a private texture
//! (outside its Direct2D draw) and wraps that as a bitmap at the canvas DPI.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows::core::Interface;
use windows::Win32::Foundation::{HANDLE, HMODULE, LUID, RECT};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::HMONITOR;

use super::backend::backoff_secs;
use crate::theme::tokens::GLASS;

/// What to capture: the canvas rectangle in screen pixels on one display.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Request {
    pub monitor: isize,
    /// x, y, w, h in physical screen pixels.
    pub rect: (i32, i32, i32, i32),
    /// Screen changes only matter inside this rectangle (the notch window plus the blur's
    /// reach); it can change every frame without restarting capture.
    pub watch: (i32, i32, i32, i32),
    /// The notch window's recent rectangles. Windows reports an excluded window's own
    /// presents as dirty rects exactly this size; those are not changes behind the glass.
    pub own: [(i32, i32, i32, i32); 4],
    /// The render device's adapter (the shared texture must live on the same GPU).
    pub adapter: (u32, i32),
    /// While the notch animates the backdrop follows at display rate, otherwise it is capped.
    pub animating: bool,
    /// The refresh cap while the notch is still (Glass is calmer than Liquid Glass).
    pub hz: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Starting,
    Running,
    /// Capture is unavailable for now (the reason, for logs).
    Failed(&'static str),
}

/// A shared texture the render device can open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Published {
    pub handle: isize,
    pub w: u32,
    pub h: u32,
    pub gen: u64,
}

struct Shared {
    req: Mutex<Option<Request>>,
    stop: AtomicBool,
    status: Mutex<Status>,
    published: Mutex<Option<Published>>,
    frames: AtomicU64,
    wake: Box<dyn Fn() + Send + Sync>,
}

pub struct Capture {
    shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Capture {
    /// Starts the worker; `wake` is called (from the worker) when a new frame is ready.
    pub fn start(wake: impl Fn() + Send + Sync + 'static) -> Self {
        let shared = Arc::new(Shared {
            req: Mutex::new(None),
            stop: AtomicBool::new(false),
            status: Mutex::new(Status::Starting),
            published: Mutex::new(None),
            frames: AtomicU64::new(0),
            wake: Box::new(wake),
        });
        let s = shared.clone();
        let thread = std::thread::Builder::new()
            .name("glass-capture".into())
            .spawn(move || worker(s))
            .ok();
        Self { shared, thread }
    }

    pub fn set_request(&self, r: Request) {
        if let Ok(mut q) = self.shared.req.lock() {
            if *q != Some(r) {
                *q = Some(r);
            }
        }
    }

    pub fn status(&self) -> Status {
        self.shared
            .status
            .lock()
            .map(|s| *s)
            .unwrap_or(Status::Failed("poisoned"))
    }

    pub fn published(&self) -> Option<Published> {
        self.shared.published.lock().ok().and_then(|p| *p)
    }

    /// Frames copied so far (for --perf).
    pub fn frames(&self) -> u64 {
        self.shared.frames.load(Ordering::Relaxed)
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn set_status(s: &Shared, st: Status) {
    if let Ok(mut g) = s.status.lock() {
        *g = st;
    }
}

fn current(s: &Shared) -> Option<Request> {
    s.req.lock().ok().and_then(|r| *r)
}

/// Sleeps up to `d`, waking early on stop or a changed request.
fn nap(s: &Shared, d: Duration, req: Option<Request>) {
    let until = Instant::now() + d;
    while Instant::now() < until {
        if s.stop.load(Ordering::Relaxed) {
            return;
        }
        let now = current(s);
        if now.map(|r| (r.monitor, r.rect, r.adapter))
            != req.map(|r| (r.monitor, r.rect, r.adapter))
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn worker(s: Arc<Shared>) {
    let mut failures = 0u32;
    let mut gen = 0u64;
    while !s.stop.load(Ordering::Relaxed) {
        let Some(req) = current(&s) else {
            std::thread::sleep(Duration::from_millis(50));
            continue;
        };
        gen += 1;
        match Session::open(&req, gen) {
            Ok(sess) => {
                if let Ok(mut p) = s.published.lock() {
                    *p = Some(sess.published);
                }
                match sess.run(&s, req) {
                    Ok(()) => failures = 0,
                    Err(e) => {
                        set_status(&s, Status::Failed(e));
                        failures += 1;
                        nap(&s, Duration::from_secs(backoff_secs(failures)), Some(req));
                    }
                }
            }
            Err(e) => {
                set_status(&s, Status::Failed(e));
                if let Ok(mut p) = s.published.lock() {
                    *p = None;
                }
                failures += 1;
                nap(&s, Duration::from_secs(backoff_secs(failures)), Some(req));
            }
        }
    }
}

struct Session {
    ctx: ID3D11DeviceContext,
    dup: IDXGIOutputDuplication,
    shared: ID3D11Texture2D,
    mutex: IDXGIKeyedMutex,
    /// The newest region, and the last one handed to the renderer.
    cand: ID3D11Texture2D,
    last: ID3D11Texture2D,
    differ: Option<Differ>,
    /// The output's desktop rectangle (screen px).
    desk: RECT,
    published: Published,
    _dev: ID3D11Device,
}

/// Tells whether the region under the notch really changed, on the GPU: the difference of
/// the new and previous region goes through a histogram effect and only the histogram (an
/// aggregate with no spatial content) is read back. Windows recomposes the whole capture
/// image whenever an excluded window presents, so dirty rects alone can't tell.
struct Differ {
    ctx: windows::Win32::Graphics::Direct2D::ID2D1DeviceContext,
    target: windows::Win32::Graphics::Direct2D::ID2D1Bitmap1,
    out: windows::Win32::Graphics::Direct2D::ID2D1Image,
    hist: windows::Win32::Graphics::Direct2D::ID2D1Effect,
    have_last: bool,
    pixels: f32,
}

const BINS: usize = 64;

impl Differ {
    fn new(
        dev: &ID3D11Device,
        cand: &ID3D11Texture2D,
        last: &ID3D11Texture2D,
        scratch: &ID3D11Texture2D,
    ) -> Option<Self> {
        use windows::Win32::Graphics::Direct2D::Common::*;
        use windows::Win32::Graphics::Direct2D::*;
        unsafe {
            let factory: ID2D1Factory1 =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).ok()?;
            let dxgi: IDXGIDevice = dev.cast().ok()?;
            let d2d = factory.CreateDevice(&dxgi).ok()?;
            let ctx = d2d
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)
                .ok()?;
            let props = |target: bool| D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: if target {
                    D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW
                } else {
                    D2D1_BITMAP_OPTIONS_NONE
                },
                colorContext: std::mem::ManuallyDrop::new(None),
            };
            let bmp = |t: &ID3D11Texture2D, target: bool| -> Option<ID2D1Bitmap1> {
                let s: IDXGISurface = t.cast().ok()?;
                ctx.CreateBitmapFromDxgiSurface(&s, Some(&props(target)))
                    .ok()
            };
            let (a, b, target) = (bmp(cand, false)?, bmp(last, false)?, bmp(scratch, true)?);
            let blend = ctx.CreateEffect(&CLSID_D2D1Blend).ok()?;
            blend.SetInput(0, &b, true);
            blend.SetInput(1, &a, true);
            blend
                .SetValue(
                    D2D1_BLEND_PROP_MODE.0 as u32,
                    D2D1_PROPERTY_TYPE_ENUM,
                    &(D2D1_BLEND_MODE_DIFFERENCE.0 as u32).to_le_bytes(),
                )
                .ok()?;
            // |dr| + |dg| + |db| into red, opaque
            let sum = ctx.CreateEffect(&CLSID_D2D1ColorMatrix).ok()?;
            sum.SetInput(0, &blend.GetOutput().ok()?, true);
            let m: [f32; 20] = [
                1.0, 0.0, 0.0, 0.0, //
                1.0, 0.0, 0.0, 0.0, //
                1.0, 0.0, 0.0, 0.0, //
                0.0, 0.0, 0.0, 0.0, //
                0.0, 0.0, 0.0, 1.0,
            ];
            let mb: Vec<u8> = m.iter().flat_map(|f| f.to_le_bytes()).collect();
            sum.SetValue(
                D2D1_COLORMATRIX_PROP_COLOR_MATRIX.0 as u32,
                D2D1_PROPERTY_TYPE_MATRIX_5X4,
                &mb,
            )
            .ok()?;
            let _ = sum.SetValue(
                D2D1_COLORMATRIX_PROP_CLAMP_OUTPUT.0 as u32,
                D2D1_PROPERTY_TYPE_BOOL,
                &1u32.to_le_bytes(),
            );
            let hist = ctx.CreateEffect(&CLSID_D2D1Histogram).ok()?;
            hist.SetInput(0, &sum.GetOutput().ok()?, true);
            hist.SetValue(
                D2D1_HISTOGRAM_PROP_NUM_BINS.0 as u32,
                D2D1_PROPERTY_TYPE_UINT32,
                &(BINS as u32).to_le_bytes(),
            )
            .ok()?;
            hist.SetValue(
                D2D1_HISTOGRAM_PROP_CHANNEL_SELECT.0 as u32,
                D2D1_PROPERTY_TYPE_ENUM,
                &(D2D1_CHANNEL_SELECTOR_R.0 as u32).to_le_bytes(),
            )
            .ok()?;
            let out = hist.GetOutput().ok()?;
            let mut d = D3D11_TEXTURE2D_DESC::default();
            cand.GetDesc(&mut d);
            Some(Self {
                ctx,
                target,
                out,
                hist,
                have_last: false,
                pixels: (d.Width * d.Height) as f32,
            })
        }
    }

    /// Whether `cand` differs from `last` (beyond a few units of noise).
    fn changed(&mut self) -> bool {
        use windows::Win32::Graphics::Direct2D::*;
        if !self.have_last {
            self.have_last = true;
            return true;
        }
        unsafe {
            self.ctx.SetTarget(&self.target);
            self.ctx.BeginDraw();
            self.ctx.DrawImage(
                &self.out,
                None,
                None,
                D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                windows::Win32::Graphics::Direct2D::Common::D2D1_COMPOSITE_MODE_SOURCE_COPY,
            );
            if self.ctx.EndDraw(None, None).is_err() {
                return true;
            }
            self.ctx.SetTarget(None);
            let mut bins = [0u8; BINS * 4];
            if self
                .hist
                .GetValue(
                    D2D1_HISTOGRAM_PROP_HISTOGRAM_OUTPUT.0 as u32,
                    D2D1_PROPERTY_TYPE_BLOB,
                    &mut bins,
                )
                .is_err()
            {
                return true;
            }
            let zero = f32::from_le_bytes([bins[0], bins[1], bins[2], bins[3]]);
            // a single changed pixel moves the first bin by 1/pixels
            zero < 1.0 - 0.5 / self.pixels.max(1.0)
        }
    }
}

fn find_output(monitor: isize) -> Option<(IDXGIAdapter1, IDXGIOutput)> {
    unsafe {
        let f: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let mut i = 0;
        while let Ok(a) = f.EnumAdapters1(i) {
            let mut j = 0;
            while let Ok(o) = a.EnumOutputs(j) {
                if let Ok(d) = o.GetDesc() {
                    if d.Monitor == HMONITOR(monitor as _) {
                        return Some((a, o));
                    }
                }
                j += 1;
            }
            i += 1;
        }
        None
    }
}

impl Session {
    fn open(req: &Request, gen: u64) -> Result<Session, &'static str> {
        let (adapter, output) = find_output(req.monitor).ok_or("display not found")?;
        unsafe {
            let ad = adapter.GetDesc1().map_err(|_| "adapter")?;
            if (ad.AdapterLuid.LowPart, ad.AdapterLuid.HighPart) != req.adapter {
                return Err("another GPU drives this display");
            }
            let mut dev = None;
            let mut ctx = None;
            D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut dev),
                None,
                Some(&mut ctx),
            )
            .map_err(|_| "capture device")?;
            let dev = dev.ok_or("capture device")?;
            let ctx = ctx.ok_or("capture device")?;
            let o1: IDXGIOutput1 = output.cast().map_err(|_| "output")?;
            let dup = o1.DuplicateOutput(&dev).map_err(|e| {
                if e.code() == windows::Win32::Foundation::E_ACCESSDENIED {
                    "secure desktop"
                } else if e.code() == DXGI_ERROR_NOT_CURRENTLY_AVAILABLE {
                    "too many screen captures"
                } else if e.code() == DXGI_ERROR_UNSUPPORTED {
                    "duplication unsupported"
                } else {
                    "duplication failed"
                }
            })?;
            let dd = dup.GetDesc();
            if dd.Rotation != DXGI_MODE_ROTATION_IDENTITY
                && dd.Rotation != DXGI_MODE_ROTATION_UNSPECIFIED
            {
                return Err("rotated display");
            }
            let desk = output.GetDesc().map_err(|_| "output")?.DesktopCoordinates;
            let (w, h) = (req.rect.2.max(1) as u32, req.rect.3.max(1) as u32);
            let desc = D3D11_TEXTURE2D_DESC {
                Width: w,
                Height: h,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
                CPUAccessFlags: 0,
                MiscFlags: D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX.0 as u32,
            };
            let mut t = None;
            dev.CreateTexture2D(&desc, None, Some(&mut t))
                .map_err(|_| "texture")?;
            let shared = t.ok_or("texture")?;
            // start black (parts off the display stay black)
            let mut rtv = None;
            if dev
                .CreateRenderTargetView(&shared, None, Some(&mut rtv))
                .is_ok()
            {
                if let Some(rtv) = rtv {
                    ctx.ClearRenderTargetView(&rtv, &[0.0, 0.0, 0.0, 1.0]);
                }
            }
            let mutex: IDXGIKeyedMutex = shared.cast().map_err(|_| "mutex")?;
            let res: IDXGIResource = shared.cast().map_err(|_| "share")?;
            let handle = res.GetSharedHandle().map_err(|_| "share")?;
            let plain = D3D11_TEXTURE2D_DESC {
                MiscFlags: 0,
                ..desc
            };
            let mk = || -> Result<ID3D11Texture2D, &'static str> {
                let mut t = None;
                dev.CreateTexture2D(&plain, None, Some(&mut t))
                    .map_err(|_| "texture")?;
                let t = t.ok_or("texture")?;
                let mut rtv = None;
                if dev.CreateRenderTargetView(&t, None, Some(&mut rtv)).is_ok() {
                    if let Some(rtv) = rtv {
                        ctx.ClearRenderTargetView(&rtv, &[0.0, 0.0, 0.0, 1.0]);
                    }
                }
                Ok(t)
            };
            let (cand, last, scratch) = (mk()?, mk()?, mk()?);
            let differ = Differ::new(&dev, &cand, &last, &scratch);
            Ok(Session {
                ctx,
                dup,
                shared,
                mutex,
                cand,
                last,
                differ,
                desk,
                published: Published {
                    handle: handle.0 as isize,
                    w,
                    h,
                    gen,
                },
                _dev: dev,
            })
        }
    }

    /// Captures until the request changes (Ok), or capture breaks (Err).
    fn run(mut self, s: &Shared, req: Request) -> Result<(), &'static str> {
        set_status(s, Status::Running);
        let (rx, ry, rw, rh) = req.rect;
        // the part of the request on this display, and where it lands in the texture
        let ix0 = rx.max(self.desk.left);
        let iy0 = ry.max(self.desk.top);
        let ix1 = (rx + rw).min(self.desk.right);
        let iy1 = (ry + rh).min(self.desk.bottom);
        if ix1 <= ix0 || iy1 <= iy0 {
            return Err("notch is off this display");
        }
        let src = D3D11_BOX {
            left: (ix0 - self.desk.left) as u32,
            top: (iy0 - self.desk.top) as u32,
            front: 0,
            right: (ix1 - self.desk.left) as u32,
            bottom: (iy1 - self.desk.top) as u32,
            back: 1,
        };
        let (dx, dy) = ((ix0 - rx) as u32, (iy0 - ry) as u32);
        let mut held = false;
        let mut first = true;
        // a changed region the renderer hasn't taken yet
        let mut pending = false;
        let mut last_check = Instant::now() - Duration::from_secs(1);
        let mut quiet = 0u32;
        let mut rects: Vec<RECT> = Vec::new();
        let result = loop {
            if s.stop.load(Ordering::Relaxed) {
                break Ok(());
            }
            let now = current(s);
            let changed = now.map(|r| (r.monitor, r.rect, r.adapter))
                != Some((req.monitor, req.rect, req.adapter));
            if changed {
                break Ok(());
            }
            let animating = now.is_some_and(|r| r.animating);
            unsafe {
                // a change the renderer hasn't taken yet: retry the hand-over
                if pending && self.publish(5) {
                    pending = false;
                    s.frames.fetch_add(1, Ordering::Relaxed);
                    (s.wake)();
                }
                if held {
                    let _ = self.dup.ReleaseFrame();
                    held = false;
                }
                // cap the refresh while the notch is still (the desktop accumulates meanwhile);
                // while nothing behind it changes, look less and less often (up to 5 Hz)
                if !animating && !first {
                    let hz = now
                        .map(|r| r.hz)
                        .unwrap_or(GLASS.backdrop_hz_static)
                        .max(1.0);
                    let min = Duration::from_secs_f32((1 + quiet.min(5)) as f32 / hz);
                    let since = last_check.elapsed();
                    if since < min {
                        std::thread::sleep(min - since);
                    }
                }
                let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
                let mut res: Option<IDXGIResource> = None;
                match self.dup.AcquireNextFrame(100, &mut info, &mut res) {
                    Ok(()) => held = true,
                    Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => continue,
                    Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST => break Err("access lost"),
                    Err(_) => break Err("acquire failed"),
                }
                if info.LastPresentTime == 0 && !first {
                    continue; // pointer-only update
                }
                let watch = now.map(|r| r.watch).unwrap_or(req.rect);
                let wbox = D3D11_BOX {
                    left: (watch.0 - self.desk.left).max(0) as u32,
                    top: (watch.1 - self.desk.top).max(0) as u32,
                    front: 0,
                    right: (watch.0 + watch.2 - self.desk.left).max(0) as u32,
                    bottom: (watch.1 + watch.3 - self.desk.top).max(0) as u32,
                    back: 1,
                };
                let own = now.map(|r| r.own).unwrap_or_default();
                if !first && !self.touches(&info, &wbox, &own, &mut rects) {
                    continue;
                }
                let Some(img) = res.and_then(|r| r.cast::<ID3D11Texture2D>().ok()) else {
                    continue;
                };
                self.ctx
                    .CopySubresourceRegion(&self.cand, 0, dx, dy, 0, &img, 0, Some(&src));
                first = false;
                last_check = Instant::now();
                let changed = self.differ.as_mut().is_none_or(|d| d.changed());
                if !changed {
                    quiet += 1;
                    continue;
                }
                quiet = 0;
                self.ctx.CopyResource(&self.last, &self.cand);
                if self.publish(30) {
                    pending = false;
                    s.frames.fetch_add(1, Ordering::Relaxed);
                    (s.wake)();
                } else {
                    pending = true;
                }
            }
        };
        if held {
            unsafe {
                let _ = self.dup.ReleaseFrame();
            }
        }
        result
    }

    /// Whether this frame's dirty or move rects touch the captured region.
    unsafe fn touches(
        &self,
        info: &DXGI_OUTDUPL_FRAME_INFO,
        src: &D3D11_BOX,
        own: &[(i32, i32, i32, i32); 4],
        rects: &mut Vec<RECT>,
    ) -> bool {
        if info.TotalMetadataBufferSize == 0 {
            return info.LastPresentTime != 0;
        }
        let (dl, dt) = (self.desk.left, self.desk.top);
        let hit = |r: &RECT| {
            let ours = (r.left + dl, r.top + dt, r.right - r.left, r.bottom - r.top);
            !is_own(ours, own)
                && r.left < src.right as i32
                && r.right > src.left as i32
                && r.top < src.bottom as i32
                && r.bottom > src.top as i32
        };
        let cap = (info.TotalMetadataBufferSize as usize / std::mem::size_of::<RECT>()).max(1);
        rects.clear();
        rects.resize(cap, RECT::default());
        let mut used = 0u32;
        if unsafe {
            self.dup.GetFrameDirtyRects(
                (cap * std::mem::size_of::<RECT>()) as u32,
                rects.as_mut_ptr(),
                &mut used,
            )
        }
        .is_err()
        {
            return true;
        }
        let n = used as usize / std::mem::size_of::<RECT>();
        if std::env::var("NOTCHLY_TRACE").is_ok() {
            // geometry only (never pixels)
            let list: Vec<_> = rects[..n.min(rects.len())]
                .iter()
                .map(|r| (r.left, r.top, r.right - r.left, r.bottom - r.top))
                .collect();
            crate::util::log(&format!("capture dirty {list:?} watch {src:?}"));
        }
        if rects[..n.min(rects.len())].iter().any(hit) {
            return true;
        }
        let mut moves = vec![DXGI_OUTDUPL_MOVE_RECT::default(); cap];
        let mut used = 0u32;
        if unsafe {
            self.dup.GetFrameMoveRects(
                (cap * std::mem::size_of::<DXGI_OUTDUPL_MOVE_RECT>()) as u32,
                moves.as_mut_ptr(),
                &mut used,
            )
        }
        .is_err()
        {
            return true;
        }
        let n = used as usize / std::mem::size_of::<DXGI_OUTDUPL_MOVE_RECT>();
        moves[..n.min(moves.len())]
            .iter()
            .any(|m| hit(&m.DestinationRect))
    }

    /// Hands the newest region to the renderer once it has released the shared texture.
    fn publish(&self, wait_ms: u32) -> bool {
        if !acquire(&self.mutex, 0, wait_ms) {
            return false;
        }
        unsafe {
            self.ctx.CopyResource(&self.shared, &self.last);
            let _ = self.mutex.ReleaseSync(1);
            self.ctx.Flush();
        }
        true
    }
}

/// Whether a dirty rect (screen px) is just the notch window presenting: exactly one of its
/// recent rectangles, or the union of two consecutive ones (it moved between frames).
pub fn is_own(r: (i32, i32, i32, i32), own: &[(i32, i32, i32, i32); 4]) -> bool {
    let union = |a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)| {
        let (l, t) = (a.0.min(b.0), a.1.min(b.1));
        let (rr, bb) = ((a.0 + a.2).max(b.0 + b.2), (a.1 + a.3).max(b.1 + b.3));
        (l, t, rr - l, bb - t)
    };
    own.iter().any(|o| o.2 > 0 && *o == r)
        || own
            .windows(2)
            .any(|w| w[0].2 > 0 && w[1].2 > 0 && union(w[0], w[1]) == r)
}

/// `AcquireSync` that tells a timeout (a success HRESULT) apart from owning the mutex.
pub fn acquire(m: &IDXGIKeyedMutex, key: u64, ms: u32) -> bool {
    let hr = unsafe { (Interface::vtable(m).AcquireSync)(Interface::as_raw(m), key, ms) };
    hr.0 == 0
}

/// The render side: the shared texture opened on the render device, a private copy, and a
/// Direct2D bitmap of it at the canvas DPI.
pub struct Receiver {
    pub gen: u64,
    pub dev_gen: u64,
    pub dpi: f32,
    shared: ID3D11Texture2D,
    mutex: IDXGIKeyedMutex,
    private: ID3D11Texture2D,
    pub bitmap: windows::Win32::Graphics::Direct2D::ID2D1Bitmap1,
    /// A frame has been received at least once.
    pub ready: bool,
}

impl Receiver {
    pub fn open(
        d3d: &ID3D11Device,
        ctx: &windows::Win32::Graphics::Direct2D::ID2D1DeviceContext,
        p: Published,
        dev_gen: u64,
        dpi: f32,
    ) -> Option<Self> {
        use windows::Win32::Graphics::Direct2D::Common::*;
        use windows::Win32::Graphics::Direct2D::*;
        unsafe {
            let mut opened: Option<ID3D11Texture2D> = None;
            d3d.OpenSharedResource(HANDLE(p.handle as _), &mut opened)
                .ok()?;
            let shared = opened?;
            let mutex: IDXGIKeyedMutex = shared.cast().ok()?;
            let desc = D3D11_TEXTURE2D_DESC {
                Width: p.w,
                Height: p.h,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let mut t = None;
            d3d.CreateTexture2D(&desc, None, Some(&mut t)).ok()?;
            let private = t?;
            let surf: IDXGISurface = private.cast().ok()?;
            let props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: dpi,
                dpiY: dpi,
                bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
                colorContext: std::mem::ManuallyDrop::new(None),
            };
            let bitmap = ctx.CreateBitmapFromDxgiSurface(&surf, Some(&props)).ok()?;
            Some(Self {
                gen: p.gen,
                dev_gen,
                dpi,
                shared,
                mutex,
                private,
                bitmap,
                ready: false,
            })
        }
    }

    /// Takes a new frame if one is waiting (never blocks). Call outside BeginDraw/EndDraw.
    pub fn take(&mut self, d3d: &ID3D11Device) -> bool {
        if !acquire(&self.mutex, 1, 0) {
            return false;
        }
        unsafe {
            if let Ok(ctx) = d3d.GetImmediateContext() {
                ctx.CopyResource(&self.private, &self.shared);
            }
            let _ = self.mutex.ReleaseSync(0);
        }
        self.ready = true;
        true
    }
}

/// The render device's adapter LUID.
pub fn adapter_luid(dxgi: &IDXGIDevice) -> Option<(u32, i32)> {
    unsafe {
        let a = dxgi.GetAdapter().ok()?;
        let d = a.GetDesc().ok()?;
        let LUID { LowPart, HighPart } = d.AdapterLuid;
        Some((LowPart, HighPart))
    }
}

#[cfg(test)]
mod tests {
    use super::is_own;

    #[test]
    fn own_presents_are_not_backdrop_changes() {
        let own = [
            (339, 0, 244, 46),
            (338, 0, 246, 46),
            (0, 0, 0, 0),
            (0, 0, 0, 0),
        ];
        assert!(is_own((339, 0, 244, 46), &own));
        assert!(is_own((338, 0, 246, 46), &own));
        // the window moved between frames: the union of both rects
        assert!(is_own((338, 0, 246, 46), &own));
        // a video playing under the notch has its own rectangle
        assert!(!is_own((400, 10, 120, 30), &own));
        // a real change merged with ours is bigger than either: still counted
        assert!(!is_own((300, 0, 300, 80), &own));
        // empty slots never match
        assert!(!is_own((0, 0, 0, 0), &own));
    }
}
