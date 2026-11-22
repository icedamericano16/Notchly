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

/// Draws the Notchly mark (a black pill with a cyan→violet glow line) at 16/32/48/256 px.
fn make_ico() -> Vec<u8> {
    let sizes = [16u32, 32, 48, 256];
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
    let f = s as f32;
    // bottom-up rows
    for row in (0..s).rev() {
        for col in 0..s {
            let (x, y) = (col as f32 + 0.5, row as f32 + 0.5);
            // rounded pill covering most of the icon (matches packaging/make_assets.py)
            let (cx, cy, hw, hh) = (f / 2.0, f / 2.0, f * 0.47, f * 0.30);
            let r = hh;
            let dx = ((x - cx).abs() - (hw - r)).max(0.0);
            let dy = ((y - cy).abs() - (hh - r)).max(0.0);
            let d = (dx * dx + dy * dy).sqrt() - r;
            let cov = (0.5 - d).clamp(0.0, 1.0);
            // near-black body with a faint top sheen
            let sheen = (1.0 - (y - (cy - hh)) / (hh * 1.2)).max(0.0) * 18.0;
            let (mut r8, mut g8, mut b8) = (8.0 + sheen, 9.0 + sheen, 12.0 + sheen);
            // anti-aliased glow line (a capsule) along the lower third
            let ly = cy + hh * 0.38;
            let half = hw * 0.52;
            let lw = (f * 0.06).max(1.2);
            let px = (x - cx).clamp(-half, half);
            let ld = ((x - cx - px).powi(2) + (y - ly).powi(2)).sqrt() - lw / 2.0;
            let la = (0.5 - ld).clamp(0.0, 1.0);
            if la > 0.0 {
                let (gr, gg, gb) = lerp3(((x - (cx - half)) / (2.0 * half)).clamp(0.0, 1.0));
                r8 += (gr - r8) * la;
                g8 += (gg - g8) * la;
                b8 += (gb - b8) * la;
            }
            let a = cov;
            v.extend_from_slice(&[
                (b8 * a) as u8,
                (g8 * a) as u8,
                (r8 * a) as u8,
                (a * 255.0) as u8,
            ]);
        }
    }
    let mask_row = s.div_ceil(32) * 4;
    v.extend(std::iter::repeat_n(0u8, (mask_row * s) as usize));
    v
}

fn lerp3(t: f32) -> (f32, f32, f32) {
    let a = (25.0, 184.0, 255.0);
    let b = (45.0, 91.0, 255.0);
    let c = (123.0, 75.0, 255.0);
    let l = |p: (f32, f32, f32), q: (f32, f32, f32), u: f32| {
        (
            p.0 + (q.0 - p.0) * u,
            p.1 + (q.1 - p.1) * u,
            p.2 + (q.2 - p.2) * u,
        )
    };
    if t < 0.55 {
        l(a, b, t / 0.55)
    } else {
        l(b, c, (t - 0.55) / 0.45)
    }
}

/// Reads `notchly.local.toml` (git-ignored, `KEY = "value"` lines) and passes each entry to the
/// compiler for `option_env!`, unless the same variable is already set in the environment.
fn load_local_settings() {
    let path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("notchly.local.toml");
    println!("cargo:rerun-if-changed={}", path.display());
    let Ok(text) = fs::read_to_string(&path) else {
        return;
    };
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
        if k.starts_with("NOTCHLY_") && !v.is_empty() && env::var(k).map_or(true, |e| e.is_empty()) {
            println!("cargo:rustc-env={k}={v}");
        }
    }
}
