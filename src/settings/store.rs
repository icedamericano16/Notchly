//! Loading, saving and migrating settings.toml.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use super::model::{Settings, SETTINGS_VERSION};
use crate::util::{atomic_write, data_dir, log, DATA_RESET};

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.toml")
}

/// Loads settings, applying forward migrations. A corrupt file is renamed `.bad`.
pub fn load() -> Settings {
    let path = settings_path();
    let Ok(text) = fs::read_to_string(&path) else {
        return Settings::default();
    };
    match parse(&text) {
        Ok(s) => s,
        Err(e) => {
            log(&format!("settings corrupt: {e}"));
            let _ = fs::rename(&path, path.with_extension("bad"));
            DATA_RESET.store(true, Ordering::Relaxed);
            Settings::default()
        }
    }
}

/// Parses and migrates a settings document.
pub fn parse(text: &str) -> Result<Settings, String> {
    let mut value: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
    let version = value
        .get("version")
        .and_then(|v| v.as_integer())
        .unwrap_or(1) as u32;
    if version > SETTINGS_VERSION {
        log("settings from a newer version; unknown keys are ignored");
    }
    migrate(&mut value, version);
    value.insert(
        "version".into(),
        toml::Value::Integer(SETTINGS_VERSION as i64),
    );
    let mut s = match deserialize(&value) {
        Ok(s) => s,
        Err(e) => {
            // a bad value somewhere: keep everything that's fine, default the rest
            log(&format!("settings: {e}; bad values fall back to defaults"));
            salvage(&value)
        }
    };
    s.sanitize();
    Ok(s)
}

fn deserialize(t: &toml::Table) -> Result<Settings, String> {
    toml::Value::Table(t.clone())
        .try_into()
        .map_err(|e: toml::de::Error| e.to_string())
}

/// Builds settings from the defaults plus every value of `user` that is valid on its own:
/// a wrong type, an unknown option or a broken section falls back to the default, field
/// by field (nested tables like `[appearance.glass]` too).
fn salvage(user: &toml::Table) -> Settings {
    let Ok(toml::Value::Table(mut good)) = toml::Value::try_from(Settings::default()) else {
        return Settings::default();
    };
    merge(&mut good, &mut Vec::new(), user);
    deserialize(&good).unwrap_or_default()
}

fn merge(root: &mut toml::Table, path: &mut Vec<String>, user: &toml::Table) {
    for (k, v) in user {
        path.push(k.clone());
        let default_is_table = matches!(lookup(root, path), Some(toml::Value::Table(_)));
        match v {
            toml::Value::Table(t) if default_is_table => merge(root, path, t),
            _ => {
                let old = set(root, path, Some(v.clone()));
                if deserialize(root).is_err() {
                    log(&format!(
                        "settings: {} is not valid; using the default",
                        path.join(".")
                    ));
                    set(root, path, old);
                }
            }
        }
        path.pop();
    }
}

fn lookup<'a>(t: &'a toml::Table, path: &[String]) -> Option<&'a toml::Value> {
    let (last, parents) = path.split_last()?;
    let mut cur = t;
    for p in parents {
        cur = cur.get(p)?.as_table()?;
    }
    cur.get(last)
}

/// Sets (or with `None` removes) the value at `path`; returns what was there.
fn set(t: &mut toml::Table, path: &[String], v: Option<toml::Value>) -> Option<toml::Value> {
    let (last, parents) = path.split_last()?;
    let mut cur = t;
    for p in parents {
        cur = cur
            .entry(p.clone())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()?;
    }
    match v {
        Some(v) => cur.insert(last.clone(), v),
        None => cur.remove(last),
    }
}

/// Forward migrations, one step per version.
fn migrate(v: &mut toml::Table, from: u32) {
    if from < 6 {
        // v6: the three animation speeds become a percentage
        if let Some(toml::Value::Table(m)) = v.get_mut("motion") {
            if let Some(toml::Value::String(sp)) = m.remove("speed") {
                let pct = match sp.as_str() {
                    "calm" => 70,
                    "fast" => 180,
                    _ => 100,
                };
                m.insert("speed_pct".into(), toml::Value::Integer(pct));
            }
        }
    }
    if from < 5 {
        // v5: the water settings move out of [productivity] into their own [water] section
        if let Some(toml::Value::Table(p)) = v.get_mut("productivity") {
            let mut w = toml::Table::new();
            for (old, new) in [
                ("water", "reminders"),
                ("water_goal_ml", "goal_ml"),
                ("water_glass_ml", "glass_ml"),
                ("water_start_min", "start_min"),
                ("water_end_min", "end_min"),
            ] {
                if let Some(x) = p.remove(old) {
                    w.insert(new.into(), x);
                }
            }
            if !w.is_empty() {
                v.insert("water".into(), toml::Value::Table(w));
            }
        }
    }
    if from < 4 {
        // v4: `Glass` + `glass_intensity` (tint 0.5-0.9) become Liquid Glass with a strength
        // mapped linearly onto 0.35-0.85
        if let Some(toml::Value::Table(a)) = v.get_mut("appearance") {
            let gi = a.remove("glass_intensity").and_then(|x| match x {
                toml::Value::Float(f) => Some(f),
                toml::Value::Integer(i) => Some(i as f64),
                _ => None,
            });
            if a.get("material").and_then(|m| m.as_str()) == Some("glass") {
                a.insert(
                    "material".into(),
                    toml::Value::String("liquid_glass".into()),
                );
                let strength = gi.map(migrate_intensity).unwrap_or(0.5);
                let mut g = toml::Table::new();
                g.insert("strength".into(), toml::Value::Float(strength as f64));
                g.insert("preset".into(), toml::Value::String("custom".into()));
                a.insert("glass".into(), toml::Value::Table(g));
            }
        }
    }
    if from < 3 {
        // v3: Home shows a calendar card instead of the timer and water tiles
        if let Some(toml::Value::Table(w)) = v.get_mut("widgets") {
            if let Some(toml::Value::Array(tiles)) = w.get("tiles").cloned() {
                let mut ids: Vec<String> = tiles
                    .iter()
                    .filter_map(|t| t.as_str())
                    .filter(|t| *t != "timer" && *t != "water")
                    .map(String::from)
                    .collect();
                for d in ["music", "battery", "todo", "weather"] {
                    if ids.len() >= 4 {
                        break;
                    }
                    if !ids.iter().any(|i| i == d) {
                        ids.push(d.into());
                    }
                }
                w.insert(
                    "tiles".into(),
                    toml::Value::Array(ids.into_iter().map(toml::Value::String).collect()),
                );
            }
        }
    }
    if from < 2 {
        // v1 stored `hover = true/false` at the top level of [general].
        if let Some(toml::Value::Table(g)) = v.get_mut("general") {
            if let Some(toml::Value::Boolean(h)) = g.remove("hover") {
                g.insert(
                    "open_mode".into(),
                    toml::Value::String(if h { "hover" } else { "click" }.into()),
                );
            }
        }
    }
}

/// Old glass tint opacity (0.5-0.9) to Liquid Glass strength (0.35-0.85).
pub fn migrate_intensity(gi: f64) -> f32 {
    (0.35 + (gi.clamp(0.5, 0.9) - 0.5) / 0.4 * 0.5) as f32
}

pub fn save(s: &Settings) {
    match toml::to_string_pretty(s) {
        Ok(text) => {
            if let Err(e) = atomic_write(&settings_path(), text.as_bytes()) {
                log(&format!("settings save failed: {e}"));
            }
        }
        Err(e) => log(&format!("settings serialize failed: {e}")),
    }
}

fn places_path() -> PathBuf {
    data_dir().join("settings_window.toml")
}

/// Where the Settings window was last on a monitor (x, y, w, h in pixels).
pub fn window_place(monitor: &str) -> Option<[i32; 4]> {
    let text = fs::read_to_string(places_path()).ok()?;
    let t: std::collections::BTreeMap<String, [i32; 4]> = toml::from_str(&text).ok()?;
    t.get(monitor).copied()
}

/// Remembers the Settings window's place on a monitor (a small file of its own, so it
/// never ends up in exported or synced settings).
pub fn save_window_place(monitor: &str, r: [i32; 4]) {
    let mut t: std::collections::BTreeMap<String, [i32; 4]> = fs::read_to_string(places_path())
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();
    t.insert(monitor.to_string(), r);
    if let Ok(text) = toml::to_string(&t) {
        let _ = atomic_write(&places_path(), text.as_bytes());
    }
}

pub fn export_to(path: &std::path::Path, s: &Settings) -> std::io::Result<()> {
    let text = toml::to_string_pretty(s).map_err(std::io::Error::other)?;
    fs::write(path, text)
}

pub fn import_from(path: &std::path::Path) -> Result<Settings, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse(&text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::model::OpenMode;

    #[test]
    fn roundtrip_defaults() {
        let s = Settings::default();
        let text = toml::to_string_pretty(&s).unwrap();
        assert_eq!(parse(&text).unwrap(), s);
    }

    #[test]
    fn migrates_v2_tiles() {
        let s = parse(
            "version = 2\n[widgets]\ntiles = [\"music\", \"timer\", \"water\", \"battery\"]\n",
        )
        .unwrap();
        assert_eq!(s.widgets.tiles, ["music", "battery", "todo", "weather"]);
        assert!(s.widgets.home_calendar);
    }

    #[test]
    fn migrates_v1() {
        let s = parse("version = 1\n[general]\nhover = false\n").unwrap();
        assert_eq!(s.general.open_mode, OpenMode::Click);
        assert_eq!(s.version, SETTINGS_VERSION);
    }

    #[test]
    fn partial_file_uses_defaults() {
        let s = parse("[appearance]\ntheme = \"aurora\"\n").unwrap();
        assert_eq!(s.appearance.theme, "aurora");
        assert_eq!(s.productivity.focus_min, 25);
    }

    #[test]
    fn migrates_glass_to_liquid_glass() {
        use crate::settings::model::{GlassPreset, GlassSettings, Material};
        let doc = |m: &str, gi: f32| {
            format!("version = 3\n[appearance]\nmaterial = \"{m}\"\nglass_intensity = {gi}\n")
        };
        let s = parse(&doc("glass", 0.9)).unwrap();
        assert_eq!(s.appearance.material, Material::LiquidGlass);
        assert!((s.appearance.glass.strength - 0.85).abs() < 1e-4);
        assert_eq!(s.appearance.glass.preset, GlassPreset::Custom);
        let s = parse(&doc("glass", 0.5)).unwrap();
        assert!((s.appearance.glass.strength - 0.35).abs() < 1e-4);
        let s = parse(&doc("glass", 0.7)).unwrap();
        assert!((s.appearance.glass.strength - 0.6).abs() < 1e-4);
        // solid stays solid with default glass values
        let s = parse(&doc("solid", 0.6)).unwrap();
        assert_eq!(s.appearance.material, Material::Solid);
        assert_eq!(s.appearance.glass, GlassSettings::default());
        // presets set the sliders
        let mut g = GlassSettings::default();
        g.apply_preset(GlassPreset::Frosted);
        assert_eq!(g.blur, 1.0);
        assert_eq!(g.preset, GlassPreset::Frosted);
        // hand-edited out-of-range values are clamped
        let g = GlassSettings {
            strength: 7.0,
            blur: f32::NAN,
            ..GlassSettings::default()
        }
        .sanitized();
        assert_eq!((g.strength, g.blur), (1.0, 0.5));
    }

    #[test]
    fn migrates_v4_water_section() {
        use crate::settings::model::ReminderStyle;
        let s = parse(
            "version = 4\n[productivity]\nfocus_min = 30\nwater = false\nwater_goal_ml = 3000\nwater_glass_ml = 300\nwater_start_min = 480\nwater_end_min = 1200\n",
        )
        .unwrap();
        assert!(!s.water.reminders);
        assert_eq!((s.water.goal_ml, s.water.glass_ml), (3000, 300));
        assert_eq!((s.water.start_min, s.water.end_min), (480, 1200));
        // new fields get their defaults; the rest of the section is untouched
        assert_eq!(s.water.style, ReminderStyle::PopupSound);
        assert_eq!(s.water.snooze_min, 15);
        assert!(s.water.stop_at_goal);
        assert_eq!(s.productivity.focus_min, 30);
        assert_eq!(s.version, SETTINGS_VERSION);
        // and it round-trips in the new format with no water_* keys left behind
        let text = toml::to_string_pretty(&s).unwrap();
        assert!(!text.contains("water_goal_ml"));
        assert_eq!(parse(&text).unwrap(), s);
    }

    #[test]
    fn migrates_every_old_version() {
        // a v1 file with something from every later migration still loads
        let s = parse(
            "[general]\nhover = true\n[appearance]\nmaterial = \"glass\"\nglass_intensity = 0.9\n[productivity]\nwater_goal_ml = 2500\n[widgets]\ntiles = [\"timer\", \"music\"]\n",
        )
        .unwrap();
        assert_eq!(s.general.open_mode, OpenMode::Hover);
        assert_eq!(
            s.appearance.material,
            crate::settings::model::Material::LiquidGlass
        );
        assert_eq!(s.water.goal_ml, 2500);
        assert_eq!(s.widgets.tiles[0], "music");
    }

    #[test]
    fn migrates_v5_speed_to_percent() {
        for (old, pct) in [("calm", 70), ("default", 100), ("fast", 180)] {
            let s = parse(&format!("version = 5\n[motion]\nspeed = \"{old}\"\n")).unwrap();
            assert_eq!(s.motion.speed_pct, pct);
        }
        // fluid settings arrive with defaults; a bad style falls back to Still
        let s =
            parse("version = 5\n[appearance.fluid]\nstyle = \"plasma\"\nspeed = 9.0\n").unwrap();
        assert_eq!(s.appearance.fluid.style, "still");
        assert_eq!(s.appearance.fluid.speed, 2.0);
    }

    #[test]
    fn bad_values_fall_back_to_defaults() {
        let d = Settings::default();
        let s = parse(
            "[general]\nhover_delay_ms = \"fast\"\nlanguage = \"hi\"\n[appearance]\nmaterial = \"plasma\"\ntheme = \"aurora\"\n[appearance.glass]\nblur = \"lots\"\nrim_light = 0.9\n[water]\ngoal_ml = -5\nglass_ml = 300\n",
        )
        .unwrap();
        // the bad ones are defaults ...
        assert_eq!(s.general.hover_delay_ms, d.general.hover_delay_ms);
        assert_eq!(s.appearance.material, d.appearance.material);
        assert_eq!(s.appearance.glass.blur, d.appearance.glass.blur);
        assert_eq!(s.water.goal_ml, d.water.goal_ml);
        // ... and their good neighbours are kept
        assert_eq!(s.general.language, "hi");
        assert_eq!(s.appearance.theme, "aurora");
        assert!((s.appearance.glass.rim_light - 0.9).abs() < 1e-6);
        assert_eq!(s.water.glass_ml, 300);
        // a whole section of the wrong type
        let s = parse("general = 5\n[music]\nlyrics = false\n").unwrap();
        assert_eq!(s.general, d.general);
        assert!(!s.music.lyrics);
    }

    #[test]
    fn out_of_range_values_are_repaired() {
        let d = Settings::default();
        let s = parse(
            "[general]\nhover_delay_ms = 99999\nlong_press_ms = 10\nlanguage = \"\"\n[appearance]\nsize = 333\n[water]\nstart_min = 1300\nend_min = 600\nsnooze_min = 7\n[shortcuts]\ntoggle = \"\"\n[weather]\nlat = 500.0\n",
        )
        .unwrap();
        assert_eq!(s.general.hover_delay_ms, 500);
        assert_eq!(s.general.long_press_ms, 300);
        assert_eq!(s.general.language, "en");
        assert_eq!(s.appearance.size, 0);
        assert_eq!(
            (s.water.start_min, s.water.end_min),
            (d.water.start_min, d.water.end_min)
        );
        assert_eq!(s.water.snooze_min, 15);
        assert_eq!(s.shortcuts.toggle, d.shortcuts.toggle);
        assert_eq!(s.weather.lat, 0.0);
    }

    #[test]
    fn reset_keeps_what_is_not_a_preference() {
        let mut s = Settings::default();
        s.general.onboarded = true;
        s.general.hover_delay_ms = 300;
        s.appearance.floating_pos.insert("m1".into(), 0.3);
        s.reset_all();
        assert!(s.general.onboarded);
        assert_eq!(s.general.hover_delay_ms, 110);
        assert_eq!(s.appearance.floating_pos.get("m1"), Some(&0.3));
    }

    #[test]
    fn export_import_round_trip() {
        let dir = std::env::temp_dir().join(format!("notchly-settings-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let p = dir.join("export.toml");
        let mut s = Settings::default();
        s.water.goal_ml = 3100;
        s.appearance.theme = "aurora".into();
        export_to(&p, &s).unwrap();
        assert_eq!(import_from(&p).unwrap(), s);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_is_error() {
        assert!(parse("[general\n").is_err());
    }
}
