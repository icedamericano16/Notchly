//! GPU device, composition surfaces and device-loss recovery.

use std::mem::ManuallyDrop;

use windows::core::{Interface, Result};
use windows::Win32::Foundation::{D2DERR_RECREATE_TARGET, E_FAIL, HMODULE, HWND};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;

use super::icons::IconCache;
use super::text::TextCache;
use crate::util::log;

pub struct Device {
    pub d3d: ID3D11Device,
    pub dxgi: IDXGIDevice,
    pub d2d: ID2D1Device,
    pub ctx: ID2D1DeviceContext,
    pub dcomp: IDCompositionDevice,
    pub brush: ID2D1SolidColorBrush,
    pub warp: bool,
}

pub struct Gfx {
    pub factory: ID2D1Factory1,
    pub dev: Option<Device>,
    /// Bumped on every device (re)creation; device resources compare against it.
    pub generation: u64,
    pub text: TextCache,
    pub icons: IconCache,
    pub force_warp: bool,
    /// Round caps/joins stroke style (a factory resource, survives device loss).
    pub round_stroke: Option<ID2D1StrokeStyle>,
    /// Glass effect graph and caches (device resources, rebuilt after device loss).
    pub glass: Option<super::glass::GlassRes>,
    /// GPU frame timing for --perf (timestamp queries, read back without stalling).
    pub timing: bool,
    timer: Option<GpuTimer>,
    /// Debug builds only (QA): save the next frame of a surface as a PNG. The Liquid tier
    /// hides the notch from screenshots, so QA reads the swap chain directly.
    #[cfg(debug_assertions)]
    pub dump: Option<std::path::PathBuf>,
}

struct GpuTimer {
    gen: u64,
    ctx: ID3D11DeviceContext,
    sets: Vec<[ID3D11Query; 3]>,
    next: usize,
    inflight: std::collections::VecDeque<usize>,
    open: Option<usize>,
    total_ms: f64,
    count: u32,
}

impl GpuTimer {
    fn new(d3d: &ID3D11Device, gen: u64) -> Option<Self> {
        let q = |kind| {
            let mut out = None;
            unsafe {
                d3d.CreateQuery(
                    &D3D11_QUERY_DESC {
                        Query: kind,
                        MiscFlags: 0,
                    },
                    Some(&mut out),
                )
                .ok()?;
            }
            out
        };
        let mut sets = Vec::new();
        for _ in 0..6 {
            sets.push([
                q(D3D11_QUERY_TIMESTAMP_DISJOINT)?,
                q(D3D11_QUERY_TIMESTAMP)?,
                q(D3D11_QUERY_TIMESTAMP)?,
            ]);
        }
        Some(Self {
            gen,
            ctx: unsafe { d3d.GetImmediateContext().ok()? },
            sets,
            next: 0,
            inflight: Default::default(),
            open: None,
            total_ms: 0.0,
            count: 0,
        })
    }

    fn begin(&mut self) {
        if self.inflight.len() >= self.sets.len() - 1 {
            return;
        }
        let i = self.next;
        self.next = (self.next + 1) % self.sets.len();
        unsafe {
            self.ctx.Begin(&self.sets[i][0]);
            self.ctx.End(&self.sets[i][1]);
        }
        self.open = Some(i);
    }

    fn end(&mut self) {
        if let Some(i) = self.open.take() {
            unsafe {
                self.ctx.End(&self.sets[i][2]);
                self.ctx.End(&self.sets[i][0]);
            }
            self.inflight.push_back(i);
        }
        // collect finished measurements without waiting
        while let Some(&i) = self.inflight.front() {
            let mut dj = D3D11_QUERY_DATA_TIMESTAMP_DISJOINT::default();
            let flags = D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32;
            // GetData answers S_FALSE until the GPU is done; the crate maps that to Ok, so the
            // raw HRESULT is checked (0 = S_OK = ready)
            let ctx = &self.ctx;
            let raw = |q: &ID3D11Query, p: *mut std::ffi::c_void, n: u32| unsafe {
                (Interface::vtable(ctx).GetData)(
                    Interface::as_raw(ctx),
                    Interface::as_raw(q),
                    p,
                    n,
                    flags,
                )
            };
            if raw(
                &self.sets[i][0],
                &mut dj as *mut _ as _,
                std::mem::size_of_val(&dj) as u32,
            )
            .0 != 0
            {
                break;
            }
            let read = |q: &ID3D11Query| {
                let mut v = 0u64;
                (raw(q, &mut v as *mut _ as _, 8).0 == 0).then_some(v)
            };
            let (Some(a), Some(b)) = (read(&self.sets[i][1]), read(&self.sets[i][2])) else {
                break;
            };
            self.inflight.pop_front();
            if !dj.Disjoint.as_bool() && dj.Frequency > 0 && b > a {
                self.total_ms += (b - a) as f64 * 1000.0 / dj.Frequency as f64;
                self.count += 1;
            }
        }
    }
}

struct SurfaceRes {
    swap: IDXGISwapChain1,
    target: ID2D1Bitmap1,
    _ctarget: IDCompositionTarget,
    visual: IDCompositionVisual,
}

/// One composition surface bound to a window.
pub struct Surface {
    pub hwnd: HWND,
    /// Physical pixel size of the swap chain.
    pub w: u32,
    pub h: u32,
    pub dpi: f32,
    res: Option<SurfaceRes>,
    gen: u64,
    pub offset: (f32, f32),
}

impl Surface {
    pub fn new(hwnd: HWND, w: u32, h: u32, dpi: f32) -> Self {
        Self {
            hwnd,
            w: w.max(1),
            h: h.max(1),
            dpi,
            res: None,
            gen: 0,
            offset: (0.0, 0.0),
        }
    }
    /// Changes size/DPI; the swap chain is resized lazily on the next frame.
    pub fn resize(&mut self, w: u32, h: u32, dpi: f32) {
        if (w.max(1), h.max(1), dpi) != (self.w, self.h, self.dpi) {
            self.w = w.max(1);
            self.h = h.max(1);
            self.dpi = dpi;
            self.res = None;
        }
    }
}

impl Gfx {
    pub fn new() -> Result<Self> {
        let factory: ID2D1Factory1 =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
        let dwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        let text = TextCache::new(&dwrite);
        let props = D2D1_STROKE_STYLE_PROPERTIES {
            startCap: D2D1_CAP_STYLE_ROUND,
            endCap: D2D1_CAP_STYLE_ROUND,
            dashCap: D2D1_CAP_STYLE_ROUND,
            lineJoin: D2D1_LINE_JOIN_ROUND,
            miterLimit: 10.0,
            dashStyle: D2D1_DASH_STYLE_SOLID,
            dashOffset: 0.0,
        };
        let base: &ID2D1Factory = &factory;
        let round_stroke = unsafe { base.CreateStrokeStyle(&props, None).ok() };
        Ok(Self {
            factory,
            dev: None,
            generation: 0,
            text,
            icons: IconCache::default(),
            force_warp: false,
            round_stroke,
            glass: None,
            timing: false,
            timer: None,
            #[cfg(debug_assertions)]
            dump: None,
        })
    }

    pub fn device(&mut self) -> Option<&Device> {
        if self.dev.is_none() {
            match create_device(&self.factory, self.force_warp) {
                Ok(d) => {
                    self.generation += 1;
                    log(&format!("gpu device created (warp={})", d.warp));
                    self.dev = Some(d);
                }
                Err(e) => {
                    log(&format!("device creation failed: {e}"));
                    return None;
                }
            }
        }
        self.dev.as_ref()
    }

    /// Called when any GPU call reports device loss: everything is rebuilt next frame.
    pub fn device_lost(&mut self) {
        log("gpu device lost; rebuilding");
        self.dev = None;
        self.glass = None;
    }

    /// Prepares the surface and begins drawing. Returns the context on success.
    pub fn begin(&mut self, s: &mut Surface) -> Option<ID2D1DeviceContext> {
        let gen = {
            self.device()?;
            self.generation
        };
        if s.res.is_none() || s.gen != gen {
            s.res = None;
            let dev = self.dev.as_ref()?;
            match create_surface(dev, s) {
                Ok(r) => {
                    s.res = Some(r);
                    s.gen = gen;
                }
                Err(e) => {
                    log(&format!("surface creation failed: {e}"));
                    if is_device_lost(e.code()) {
                        self.device_lost();
                    }
                    return None;
                }
            }
        }
        let dev = self.dev.as_ref()?;
        if self.glass.as_ref().is_none_or(|g| g.gen != gen) {
            self.glass = super::glass::GlassRes::new(&dev.ctx, gen);
        }
        if self.timing && self.timer.as_ref().is_none_or(|t| t.gen != gen) {
            self.timer = GpuTimer::new(&dev.d3d, gen);
        }
        let res = s.res.as_ref()?;
        unsafe {
            dev.ctx.SetTarget(&res.target);
            dev.ctx.SetDpi(s.dpi, s.dpi);
            dev.ctx
                .SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            dev.ctx.BeginDraw();
            dev.ctx
                .SetTransform(&windows_numerics::Matrix3x2::identity());
            dev.ctx.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }));
        }
        Some(dev.ctx.clone())
    }

    /// Ends drawing, presents and commits the composition offset.
    pub fn end(&mut self, s: &mut Surface) {
        let Some(dev) = self.dev.as_ref() else { return };
        let mut lost = false;
        // Direct2D records until EndDraw submits the batch: time exactly that GPU work
        if let Some(t) = self.timer.as_mut().filter(|_| self.timing) {
            t.begin();
        }
        unsafe {
            if let Err(e) = dev.ctx.EndDraw(None, None) {
                lost = e.code() == D2DERR_RECREATE_TARGET || is_device_lost(e.code());
                if !lost {
                    log(&format!("EndDraw: {e}"));
                }
            }
            dev.ctx.SetTarget(None);
            if let Some(t) = self.timer.as_mut().filter(|_| self.timing) {
                t.end();
            }
            #[cfg(debug_assertions)]
            if let (Some(path), Some(res)) = (self.dump.take(), s.res.as_ref()) {
                if !lost {
                    dump_png(dev, &res.swap, &path);
                }
            }
            if let Some(res) = s.res.as_ref() {
                if !lost {
                    let hr = res.swap.Present(1, DXGI_PRESENT(0));
                    if is_device_lost(hr) {
                        lost = true;
                    }
                    let _ = res.visual.SetOffsetX2(s.offset.0);
                    let _ = res.visual.SetOffsetY2(s.offset.1);
                    let _ = dev.dcomp.Commit();
                }
            }
        }
        if lost {
            s.res = None;
            self.device_lost();
        }
    }

    /// Average GPU time per frame since the last call (None when not measured).
    pub fn gpu_ms(&mut self) -> Option<f32> {
        let t = self.timer.as_mut()?;
        if t.count == 0 {
            return None;
        }
        let v = (t.total_ms / t.count as f64) as f32;
        t.total_ms = 0.0;
        t.count = 0;
        Some(v)
    }

    pub fn trim(&self) {
        if let Some(dev) = &self.dev {
            unsafe {
                if let Ok(d3) = dev.d3d.cast::<IDXGIDevice3>() {
                    d3.Trim();
                }
                dev.d2d.ClearResources(0);
            }
        }
    }
}

/// QA (debug builds): the back buffer composited over dark grey, saved as an uncompressed PNG.
#[cfg(debug_assertions)]
fn dump_png(dev: &Device, swap: &IDXGISwapChain1, path: &std::path::Path) {
    unsafe {
        let Ok(buf) = swap.GetBuffer::<ID3D11Texture2D>(0) else {
            return;
        };
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        buf.GetDesc(&mut desc);
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.MiscFlags = 0;
        let mut st = None;
        if dev.d3d.CreateTexture2D(&desc, None, Some(&mut st)).is_err() {
            return;
        }
        let Some(st) = st else { return };
        let Ok(ctx) = dev.d3d.GetImmediateContext() else {
            return;
        };
        ctx.CopyResource(&st, &buf);
        let mut m = D3D11_MAPPED_SUBRESOURCE::default();
        if ctx.Map(&st, 0, D3D11_MAP_READ, 0, Some(&mut m)).is_err() {
            return;
        }
        let (w, h) = (desc.Width as usize, desc.Height as usize);
        // straight-alpha RGBA, so QA can composite the frame over a desktop screenshot
        let mut rgba = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            // SAFETY: the staging texture is mapped for reading; each of its `h` rows starts
            // RowPitch bytes apart and holds at least w * 4 bytes (B8G8R8A8). Unmapped below.
            let row = std::slice::from_raw_parts(
                (m.pData as *const u8).add(y * m.RowPitch as usize),
                w * 4,
            );
            for p in row.as_chunks::<4>().0 {
                let a = p[3] as f32 / 255.0;
                let un = |c: u8| {
                    if a > 0.0 {
                        (c as f32 / a).min(255.0) as u8
                    } else {
                        0
                    }
                };
                rgba.extend_from_slice(&[un(p[2]), un(p[1]), un(p[0]), p[3]]);
            }
        }
        ctx.Unmap(&st, 0);
        let _ = std::fs::write(path, crate::util::png_rgba(w as u32, h as u32, &rgba));
        log(&format!("qa: frame saved to {}", path.display()));
    }
}

fn is_device_lost(hr: windows::core::HRESULT) -> bool {
    hr == DXGI_ERROR_DEVICE_REMOVED || hr == DXGI_ERROR_DEVICE_RESET || hr == D2DERR_RECREATE_TARGET
}

fn create_device(factory: &ID2D1Factory1, force_warp: bool) -> Result<Device> {
    let flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;
    let mut d3d: Option<ID3D11Device> = None;
    let mut warp = force_warp;
    let hw = if force_warp {
        Err(E_FAIL.into())
    } else {
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                flags,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d),
                None,
                None,
            )
        }
    };
    if hw.is_err() || d3d.is_none() {
        warp = true;
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_WARP,
                HMODULE::default(),
                flags,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d),
                None,
                None,
            )?
        };
    }
    let d3d = d3d.ok_or_else(|| windows::core::Error::from(E_FAIL))?;
    let dxgi: IDXGIDevice = d3d.cast()?;
    let d2d = unsafe { factory.CreateDevice(&dxgi)? };
    let ctx = unsafe { d2d.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)? };
    let dcomp: IDCompositionDevice = unsafe { DCompositionCreateDevice(&dxgi)? };
    let brush = unsafe {
        ctx.CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            None,
        )?
    };
    Ok(Device {
        d3d,
        dxgi,
        d2d,
        ctx,
        dcomp,
        brush,
        warp,
    })
}

fn create_surface(dev: &Device, s: &Surface) -> Result<SurfaceRes> {
    let adapter = unsafe { dev.dxgi.GetAdapter()? };
    let factory: IDXGIFactory2 = unsafe { adapter.GetParent()? };
    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: s.w,
        Height: s.h,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
        Flags: 0,
    };
    let swap = unsafe { factory.CreateSwapChainForComposition(&dev.d3d, &desc, None)? };
    let surface: IDXGISurface = unsafe { swap.GetBuffer(0)? };
    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: s.dpi,
        dpiY: s.dpi,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        colorContext: ManuallyDrop::new(None),
    };
    let target = unsafe {
        dev.ctx
            .CreateBitmapFromDxgiSurface(&surface, Some(&props))?
    };
    let ctarget = unsafe { dev.dcomp.CreateTargetForHwnd(s.hwnd, true)? };
    let visual = unsafe { dev.dcomp.CreateVisual()? };
    unsafe {
        visual.SetContent(&swap)?;
        ctarget.SetRoot(&visual)?;
        dev.dcomp.Commit()?;
    }
    Ok(SurfaceRes {
        swap,
        target,
        _ctarget: ctarget,
        visual,
    })
}
