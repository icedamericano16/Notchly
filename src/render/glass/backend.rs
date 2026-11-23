//! Which glass the notch can show right now, and why it fell back.
//!
//! Every tier is drawn by Notchly's own compositor; Windows' system backdrop is never used
//! (it ignores the window region and fills the whole notch window with a flat grey box — see
//! DECISIONS.md, "The grey rectangle").
//!
//! * `Liquid` — the backdrop behind the notch (Desktop Duplication), blurred, lensed at the rim,
//!   colour-lifted; stretch, ripple and fluid styles.
//! * `Frosted` — the *Glass* material: the same backdrop, strongly blurred, no lens, calm.
//! * `Tinted` — no backdrop at all: the glass stack (tint, rim, depth, sheen) over a transparent
//!   window. Used when the screen can't or shouldn't be read.
//! * `Solid` — the plain notch.

use crate::settings::Material;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Liquid,
    Frosted,
    Tinted,
    Solid,
}

/// Why the active tier is not the one the user picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    BatterySaver,
    TransparencyOff,
    RemoteDesktop,
    HighContrast,
    ShownInRecordings,
    /// Excluding a window from capture needs Windows 10 2004 or later.
    WindowsTooOld,
    /// Screen capture failed (another GPU drives this display, a rotated display, a secure
    /// desktop ...); retried with backoff.
    CaptureUnavailable,
    /// The GPU device is the software rasteriser: the lens is skipped.
    SoftwareRendering,
}

impl Reason {
    /// The status line in Settings (English key; translated by the caller).
    pub fn message(self) -> &'static str {
        match self {
            Reason::BatterySaver => "battery saver is on",
            Reason::TransparencyOff => "transparency effects are off in Windows",
            Reason::RemoteDesktop => "this is a Remote Desktop session",
            Reason::HighContrast => "High Contrast is on",
            Reason::ShownInRecordings => "Notchly stays visible in screen recordings",
            Reason::WindowsTooOld => "this needs Windows 10 version 2004 or later",
            Reason::CaptureUnavailable => "screen capture isn't available on this display",
            Reason::SoftwareRendering => "the graphics card is in software mode",
        }
    }
}

impl Tier {
    pub fn name(self) -> &'static str {
        match self {
            Tier::Liquid => "Liquid Glass",
            Tier::Frosted => "Glass",
            Tier::Tinted => "Tinted glass",
            Tier::Solid => "Solid",
        }
    }
    /// Whether this tier draws from the captured backdrop.
    pub fn captures(self) -> bool {
        matches!(self, Tier::Liquid | Tier::Frosted)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Inputs {
    pub material: Material,
    pub build: u32,
    pub transparency: bool,
    pub battery_saver: bool,
    pub remote: bool,
    pub high_contrast: bool,
    pub follow_transparency: bool,
    pub follow_contrast: bool,
    pub show_in_recordings: bool,
    /// The capture backend works on this display (false while it's failing / backing off).
    pub capture_ok: bool,
    /// The render device is WARP.
    pub software: bool,
}

/// First build with `WDA_EXCLUDEFROMCAPTURE` (Windows 10 2004).
pub const BUILD_EXCLUDE: u32 = 19041;

/// The tier to use and, when it isn't what was asked for, why.
///
/// | Material | Shown | Falls back to |
/// |---|---|---|
/// | Solid | Solid | — |
/// | Glass | Frosted | Tinted → Solid |
/// | Liquid Glass | Liquid | Frosted → Tinted → Solid |
pub fn select(i: &Inputs) -> (Tier, Option<Reason>) {
    if i.material == Material::Solid {
        return (Tier::Solid, None);
    }
    let solid = if i.remote {
        Some(Reason::RemoteDesktop)
    } else if i.high_contrast && i.follow_contrast {
        Some(Reason::HighContrast)
    } else if !i.transparency && i.follow_transparency {
        Some(Reason::TransparencyOff)
    } else {
        None
    };
    if let Some(r) = solid {
        return (Tier::Solid, Some(r));
    }
    // no screen reading at all: tinted glass
    let tinted = if i.battery_saver {
        Some(Reason::BatterySaver)
    } else if i.show_in_recordings {
        Some(Reason::ShownInRecordings)
    } else if i.build < BUILD_EXCLUDE {
        Some(Reason::WindowsTooOld)
    } else if !i.capture_ok {
        Some(Reason::CaptureUnavailable)
    } else {
        None
    };
    if let Some(r) = tinted {
        return (Tier::Tinted, Some(r));
    }
    match i.material {
        Material::Solid => (Tier::Solid, None),
        Material::Glass => (Tier::Frosted, None),
        Material::LiquidGlass if i.software => (Tier::Frosted, Some(Reason::SoftwareRendering)),
        Material::LiquidGlass => (Tier::Liquid, None),
    }
}

/// Retry delay after the n-th consecutive capture failure (1, 2, 4 ... 30 s).
pub fn backoff_secs(failures: u32) -> u64 {
    (1u64 << failures.min(5)).min(30)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Inputs {
        Inputs {
            material: Material::LiquidGlass,
            build: 26100,
            transparency: true,
            battery_saver: false,
            remote: false,
            high_contrast: false,
            follow_transparency: true,
            follow_contrast: true,
            show_in_recordings: false,
            capture_ok: true,
            software: false,
        }
    }

    fn t(f: fn(&mut Inputs)) -> (Tier, Option<Reason>) {
        let mut i = base();
        f(&mut i);
        select(&i)
    }

    #[test]
    fn the_material_table() {
        assert_eq!(select(&base()), (Tier::Liquid, None));
        assert_eq!(t(|i| i.material = Material::Glass), (Tier::Frosted, None));
        assert_eq!(t(|i| i.material = Material::Solid), (Tier::Solid, None));
        // Liquid -> Glass -> Tinted -> Solid
        assert_eq!(
            t(|i| i.software = true),
            (Tier::Frosted, Some(Reason::SoftwareRendering))
        );
        assert_eq!(
            t(|i| i.capture_ok = false),
            (Tier::Tinted, Some(Reason::CaptureUnavailable))
        );
        assert_eq!(
            t(|i| i.transparency = false),
            (Tier::Solid, Some(Reason::TransparencyOff))
        );
        // Glass -> Tinted -> Solid
        assert_eq!(
            t(|i| {
                i.material = Material::Glass;
                i.capture_ok = false;
            }),
            (Tier::Tinted, Some(Reason::CaptureUnavailable))
        );
        assert_eq!(
            t(|i| {
                i.material = Material::Glass;
                i.high_contrast = true;
            }),
            (Tier::Solid, Some(Reason::HighContrast))
        );
    }

    #[test]
    fn fallback_reasons() {
        assert_eq!(
            t(|i| i.battery_saver = true),
            (Tier::Tinted, Some(Reason::BatterySaver))
        );
        assert_eq!(
            t(|i| i.show_in_recordings = true),
            (Tier::Tinted, Some(Reason::ShownInRecordings))
        );
        assert_eq!(
            t(|i| i.build = 18363),
            (Tier::Tinted, Some(Reason::WindowsTooOld))
        );
        // Windows 10 2004+ gets the real glass
        assert_eq!(t(|i| i.build = 19045), (Tier::Liquid, None));
        assert_eq!(
            t(|i| i.remote = true),
            (Tier::Solid, Some(Reason::RemoteDesktop))
        );
        // not following the Windows switches keeps the glass
        assert_eq!(
            t(|i| {
                i.transparency = false;
                i.follow_transparency = false;
            }),
            (Tier::Liquid, None)
        );
        assert_eq!(
            t(|i| {
                i.high_contrast = true;
                i.follow_contrast = false;
            }),
            (Tier::Liquid, None)
        );
    }

    #[test]
    fn only_backdrop_tiers_capture() {
        assert!(Tier::Liquid.captures() && Tier::Frosted.captures());
        assert!(!Tier::Tinted.captures() && !Tier::Solid.captures());
    }

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(backoff_secs(0), 1);
        assert_eq!(backoff_secs(3), 8);
        assert_eq!(backoff_secs(9), 30);
    }
}
