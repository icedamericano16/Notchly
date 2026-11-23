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
    let s: Settings = toml::Value::Table(value)
        .try_into()
        .map_err(|e: toml::de::Error| e.to_string())?;
    Ok(s)
}

/// Forward migrations, one step per version.
fn migrate(v: &mut toml::Table, from: u32) {
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
    fn corrupt_is_error() {
        assert!(parse("[general\n").is_err());
    }
}
