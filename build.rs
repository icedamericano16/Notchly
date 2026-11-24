//! Build script: generates the app icon procedurally, writes a resource script
//! (manifest + icon + version info) and compiles it with the Windows SDK `rc.exe`.
//! If `rc.exe` cannot be found the build still succeeds; DPI awareness is then set
//! at runtime instead (see `main.rs`).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    for v in [
        "NOTCHLY_UPDATE_FEED",
        "NOTCHLY_WEBSITE",
        "NOTCHLY_SUPPORT_URL",
        "NOTCHLY_GOOGLE_CLIENT_ID",
        "NOTCHLY_GOOGLE_CLIENT_SECRET",
        "NOTCHLY_MS_CLIENT_ID",
        "NOTCHLY_UPDATE_PUBKEY",
        "NOTCHLY_PUBLISHER",
    ] {
        println!("cargo:rerun-if-env-changed={v}");
    }
    println!("cargo:rerun-if-changed=packaging/notchly.manifest");
    load_local_settings();
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let ico = out.join("notchly.ico");
    fs::write(&ico, make_ico()).unwrap();

    let template =
        Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("packaging/notchly.manifest");
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let parts: Vec<&str> = version.split('.').collect();
    // the manifest carries the version and, for signed releases, the sparse-package identity
    // (NOTCHLY_PUBLISHER = the signing certificate subject, e.g. "CN=Contoso Ltd") so the
    // installer-registered identity package attaches to this exe (notification mirroring)
    let msix = match env::var("NOTCHLY_PUBLISHER") {
        Ok(p) if !p.trim().is_empty() => format!(
            r#"<msix xmlns="urn:schemas-microsoft-com:msix.v1" publisher="{}" packageName="Notchly.Sparse" applicationId="Notchly" />"#,
            p.trim().replace('&', "&amp;").replace('"', "&quot;")
        ),
        _ => String::new(),
    };
    let manifest = out.join("notchly.manifest");
    fs::write(
        &manifest,
        fs::read_to_string(&template)
            .unwrap()
            .replace(
                "@VERSION@",
                &format!(
                    "{}.{}.{}.0",
                    parts[0],
                    parts.get(1).unwrap_or(&"0"),
                    parts.get(2).unwrap_or(&"0")
                ),
            )
            .replace("<!--@MSIX@-->", &msix),
    )
    .unwrap();
    let v = format!(
        "{},{},{},0",
        parts[0],
        parts.get(1).unwrap_or(&"0"),
        parts.get(2).unwrap_or(&"0")
    );
    let rc = format!(
        r#"#pragma code_page(65001)
1 ICON "{ico}"
1 24 "{man}"
1 VERSIONINFO
FILEVERSION {v}
PRODUCTVERSION {v}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "Notchly"
      VALUE "FileDescription", "Notchly"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "notchly"
      VALUE "OriginalFilename", "notchly.exe"
      VALUE "ProductName", "Notchly"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        ico = ico.display().to_string().replace('\\', "\\\\"),
        man = manifest.display().to_string().replace('\\', "\\\\"),
    );
    let rc_path = out.join("notchly.rc");
    fs::write(&rc_path, rc).unwrap();
    let res = out.join("notchly.res");
    if let Some(rc_exe) = find_rc() {
        let status = Command::new(rc_exe)
            .arg("/nologo")
            .arg("/fo")
            .arg(&res)
            .arg(&rc_path)
            .status();
        if matches!(status, Ok(s) if s.success()) {
            println!("cargo:rustc-link-arg-bins={}", res.display());
            return;
        }
    }
    println!("cargo:warning=rc.exe not found; building without embedded resources");
}

fn find_rc() -> Option<PathBuf> {
    let base = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut versions: Vec<PathBuf> = fs::read_dir(base)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("x64").join("rc.exe").exists())
        .collect();
    versions.sort();
    versions.pop().map(|p| p.join("x64").join("rc.exe"))
}

/// Draws the Notchly logo (assets/brand/notchly-logo.svg: a violet squircle, a black notch
/// and two eyes) at 16–256 px. `packaging/make_assets.py` draws the same mark for the MSIX.
fn make_ico() -> Vec<u8> {
    let sizes = [16u32, 24, 32, 48, 64, 256];
    let images: Vec<Vec<u8>> = sizes.iter().map(|&s| bmp_entry(s)).collect();
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (i, &s) in sizes.iter().enumerate() {
        let b = if s >= 256 { 0 } else { s as u8 };
        out.extend_from_slice(&[b, b, 0, 0, 1, 0, 32, 0]);
        out.extend_from_slice(&(images[i].len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += images[i].len() as u32;
    }
    for img in images {
        out.extend_from_slice(&img);
    }
    out
}

fn bmp_entry(s: u32) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&40u32.to_le_bytes());
    v.extend_from_slice(&(s as i32).to_le_bytes());
    v.extend_from_slice(&((s * 2) as i32).to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&32u16.to_le_bytes());
    v.extend_from_slice(&[0u8; 24]);
    // 4x4 supersampling for anti-aliased edges; BMP rows are bottom-up
    const SS: u32 = 4;
    for row in (0..s).rev() {
        for col in 0..s {
            let (mut r, mut g, mut b, mut a) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = (col as f32 + (sx as f32 + 0.5) / SS as f32) / s as f32 * 1024.0;
                    let y = (row as f32 + (sy as f32 + 0.5) / SS as f32) / s as f32 * 1024.0;
                    if let Some((cr, cg, cb)) = logo_sample(x, y) {
                        r += cr;
                        g += cg;
                        b += cb;
                        a += 1.0;
                    }
                }
            }
            let n = (SS * SS) as f32;
            // premultiplied by coverage, like the previous icon
            v.extend_from_slice(&[
                (b / n) as u8,
                (g / n) as u8,
                (r / n) as u8,
                (a / n * 255.0) as u8,
            ]);
        }
    }
    let mask_row = s.div_ceil(32) * 4;
    v.extend(std::iter::repeat_n(0u8, (mask_row * s) as usize));
    v
}

/// Colour of the logo at a point in its 1024×1024 design space, or None outside it.
/// Geometry from assets/brand/notchly-logo.svg.
fn logo_sample(x: f32, y: f32) -> Option<(f32, f32, f32)> {
    // squircle: a superellipse (n≈7) matching the SVG's smooth corners
    let (u, v) = (((x - 512.0) / 512.0).abs(), ((y - 512.0) / 512.0).abs());
    if u.powf(7.0) + v.powf(7.0) > 1.0 {
        return None;
    }
    let in_rrect = |x0: f32, y0: f32, w: f32, h: f32, r: f32| {
        if x < x0 || x > x0 + w || y < y0 || y > y0 + h {
            return false;
        }
        let cx = x.clamp(x0 + r, x0 + w - r);
        let cy = y.clamp(y0 + r, y0 + h - r);
        (x - cx).powi(2) + (y - cy).powi(2) <= r * r
    };
    if in_rrect(364.0, 383.32, 84.0, 114.0, 42.0) || in_rrect(576.0, 383.32, 84.0, 114.0, 42.0) {
        return Some((233.0, 252.0, 248.0)); // eyes #E9FCF8
    }
    if in_rrect(123.0, 302.82, 778.0, 275.0, 137.5) {
        return Some((8.0, 8.0, 12.0)); // notch #08080C
    }
    // diagonal gradient #8438FF -> #5420FF
    let t = ((x + y) / 2048.0).clamp(0.0, 1.0);
    Some((132.0 - 48.0 * t, 56.0 - 24.0 * t, 255.0))
}

/// Reads `notchly.local.toml` (or the alternative name `config.toml`): machine-local,
/// git-ignored build settings as `KEY = "value"` lines, passed to the compiler for
/// `option_env!`. Environment variables with the same names win (CI sets them from GitHub
/// variables and secrets). Every value is optional: a missing one disables its feature with a
/// clear message. See notchly.local.example.toml and docs/KEYS_AND_SECRETS.md.
fn load_local_settings() {
    let dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).to_path_buf();
    for name in ["notchly.local.toml", "config.toml"] {
        let path = dir.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        if let Ok(text) = fs::read_to_string(&path) {
            apply_local_settings(&text);
        }
    }
}

fn apply_local_settings(text: &str) {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let k = k.trim();
        let v = v.trim().trim_matches('"');
        if k.starts_with("NOTCHLY_") && !v.is_empty() && env::var(k).map_or(true, |e| e.is_empty())
        {
            println!("cargo:rustc-env={k}={v}");
        }
    }
}
