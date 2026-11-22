//! WIC decoding to premultiplied BGRA, plus device-bitmap caching.

use std::sync::Arc;

use windows::core::Result;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};

/// Decoded image: premultiplied BGRA pixels.
#[derive(Clone, Debug)]
pub struct Pixels {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

/// Decodes an encoded image (PNG/JPEG/BMP...) and downsizes it to fit `max` px.
/// Safe to call from any COM-initialised thread.
pub fn decode(bytes: &[u8], max: u32) -> Result<Pixels> {
    unsafe {
        let wic: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let stream = wic.CreateStream()?;
        stream.InitializeFromMemory(bytes)?;
        let decoder =
            wic.CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand)?;
        let frame = decoder.GetFrame(0)?;
        from_source(&wic, &frame.into(), max)
    }
}

pub fn decode_file(path: &std::path::Path, max: u32) -> Result<Pixels> {
    let bytes = std::fs::read(path)
        .map_err(|_| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?;
    decode(&bytes, max)
}

pub fn from_source(wic: &IWICImagingFactory, src: &IWICBitmapSource, max: u32) -> Result<Pixels> {
    unsafe {
        let (mut w, mut h) = (0u32, 0u32);
        src.GetSize(&mut w, &mut h)?;
        let scale = (max as f32 / w.max(h).max(1) as f32).min(1.0);
        let (tw, th) = (
            ((w as f32 * scale).round() as u32).max(1),
            ((h as f32 * scale).round() as u32).max(1),
        );
        let scaler = wic.CreateBitmapScaler()?;
        scaler.Initialize(src, tw, th, WICBitmapInterpolationModeHighQualityCubic)?;
        let conv = wic.CreateFormatConverter()?;
        conv.Initialize(
            &scaler,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeCustom,
        )?;
        let stride = tw * 4;
        let mut data = vec![0u8; (stride * th) as usize];
        conv.CopyPixels(std::ptr::null(), stride, &mut data)?;
        Ok(Pixels { w: tw, h: th, data })
    }
}

/// Encodes BGRA pixels as PNG bytes.
pub fn encode_png(px: &Pixels) -> Result<Vec<u8>> {
    use windows::Win32::System::Com::StructuredStorage::*;
    use windows::Win32::System::Com::*;
    unsafe {
        let wic: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let stream: IStream = windows::Win32::UI::Shell::SHCreateMemStream(None)
            .ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_OUTOFMEMORY))?;
        let enc = wic.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
        enc.Initialize(&stream, WICBitmapEncoderNoCache)?;
        let mut frame = None;
        let mut bag: Option<IPropertyBag2> = None;
        enc.CreateNewFrame(&mut frame, &mut bag)?;
        let frame =
            frame.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?;
        frame.Initialize(None)?;
        frame.SetSize(px.w, px.h)?;
        let mut fmt = GUID_WICPixelFormat32bppPBGRA;
        frame.SetPixelFormat(&mut fmt)?;
        frame.WritePixels(px.h, px.w * 4, &px.data)?;
        frame.Commit()?;
        enc.Commit()?;
        let mut stat = STATSTG::default();
        stream.Stat(&mut stat, STATFLAG_NONAME)?;
        let len = stat.cbSize as usize;
        stream.Seek(0, STREAM_SEEK_SET, None)?;
        let mut out = vec![0u8; len];
        let mut read = 0u32;
        stream
            .Read(out.as_mut_ptr() as _, len as u32, Some(&mut read))
            .ok()?;
        out.truncate(read as usize);
        Ok(out)
    }
}

/// A CPU image with a lazily (re)created device bitmap.
#[derive(Clone)]
pub struct CachedBitmap {
    pub pixels: Arc<Pixels>,
    bmp: Option<(u64, ID2D1Bitmap1)>,
}

impl std::fmt::Debug for CachedBitmap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CachedBitmap({}x{})", self.pixels.w, self.pixels.h)
    }
}

impl CachedBitmap {
    pub fn new(pixels: Arc<Pixels>) -> Self {
        Self { pixels, bmp: None }
    }
    pub fn get(&mut self, ctx: &ID2D1DeviceContext, gen: u64) -> Option<ID2D1Bitmap1> {
        if let Some((g, b)) = &self.bmp {
            if *g == gen {
                return Some(b.clone());
            }
        }
        let p = &self.pixels;
        let props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            colorContext: std::mem::ManuallyDrop::new(None),
        };
        let b = unsafe {
            ctx.CreateBitmap(
                D2D_SIZE_U {
                    width: p.w,
                    height: p.h,
                },
                Some(p.data.as_ptr() as _),
                p.w * 4,
                &props,
            )
            .ok()?
        };
        self.bmp = Some((gen, b.clone()));
        Some(b)
    }
}
