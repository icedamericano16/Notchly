//! Settings › Theme Studio: the theme gallery and the editor rows. Edits go to the active
//! theme; a built-in is copied first ("Made a copy you can edit"). Every edit applies live,
//! is saved to the theme's file (debounced) and can be undone.

use super::window::{card, it, when, Ctl, Group, Section};
use crate::app::state::State;
use crate::i18n::tr;
use crate::render::icons::Icon;
use crate::theme::studio::{self, copy_name, hex, pack_of, unique_id};
use crate::theme::tokens::*;
use crate::theme::{self, Theme, ThemePack};

/// Every theme the gallery and the dropdowns offer: presets, then packs (built-in and yours).
pub fn all_themes(s: &State) -> Vec<Theme> {
    theme::PRESETS
        .iter()
        .filter_map(|p| theme::preset(p.id))
        .chain(s.packs.iter().cloned())
        .collect()
}

fn taken(s: &State) -> Vec<String> {
    all_themes(s).into_iter().map(|t| t.id).collect()
}

/// The active theme as an editable pack.
pub fn current(s: &State) -> ThemePack {
    s.studio
        .find(s.theme_id())
        .cloned()
        .unwrap_or_else(|| pack_of(&theme::resolve(s.theme_id(), &s.packs)))
}

pub fn editable(s: &State) -> bool {
    s.studio.is_user(s.theme_id())
}

/// Packs in memory follow the user's edits at once (files are written later).
fn refresh_packs(s: &mut State) {
    let mut packs: Vec<Theme> = theme::BUILTIN_PACKS
        .iter()
        .filter_map(|j| serde_json::from_str::<ThemePack>(j).ok())
        .filter_map(|p| p.to_theme().ok())
        .collect();
    packs.extend(s.studio.user.iter().filter_map(|p| p.to_theme().ok()));
    s.packs = packs;
}

/// Makes `id` the theme for the current mode, with the glass defaults it brings.
pub fn apply_theme(s: &mut State, id: &str) {
    if s.light_mode() {
        s.settings.appearance.theme_light = id.to_string();
    } else {
        s.settings.appearance.theme = id.to_string();
    }
    let t = theme::resolve(id, &s.packs);
    let g = &mut s.settings.appearance.glass;
    if let Some(v) = t.glass_strength {
        g.strength = v;
        g.preset = crate::settings::GlassPreset::Custom;
    }
    if let Some(v) = t.glass_blur {
        g.blur = v;
        g.preset = crate::settings::GlassPreset::Custom;
    }
    s.apply_settings();
    s.save_settings_soon();
}

fn commit(s: &mut State, p: ThemePack) {
    let id = format!("pack:{}", p.id);
    s.studio.store(p);
    refresh_packs(s);
    apply_theme(s, &id);
    s.dirty = true;
}

/// Edits the active theme (a built-in is copied first). Invalid results are dropped.
pub fn edit(s: &mut State, f: impl FnOnce(&mut ThemePack)) {
    let before = current(s);
    let mut p = before.clone();
    if editable(s) {
        s.studio.checkpoint(&before);
    } else {
        let names: Vec<String> = s.studio.user.iter().map(|u| u.name.clone()).collect();
        p.name = copy_name(&p.name, &names);
        p.id = unique_id(&p.name, &taken(s));
        s.studio.notice = Some(tr("Made a copy you can edit"));
    }
    p.version = Some(2);
    f(&mut p);
    if p.to_theme().is_ok() {
        commit(s, p);
    }
}

/// A theme from a file or a share code joins your themes (renamed if the id is taken).
/// Gallery themes (assets/themes/gallery.txt) that aren't installed yet, in file order.
pub fn gallery_extras(s: &State) -> Vec<ThemePack> {
    let have = taken(s);
    theme::gallery::packs()
        .iter()
        .filter(|p| !have.iter().any(|t| t.trim_start_matches("pack:") == p.id))
        .cloned()
        .collect()
}

/// Installs the `i`-th not-yet-installed gallery theme and applies it.
pub fn install_gallery(s: &mut State, i: usize) -> String {
    match gallery_extras(s).get(i).cloned() {
        Some(p) => add_pack(s, p),
        None => String::new(),
    }
}

fn add_pack(s: &mut State, mut p: ThemePack) -> String {
    if taken(s)
        .iter()
        .any(|t| t.trim_start_matches("pack:") == p.id)
    {
        p.id = unique_id(&p.name, &taken(s));
    }
    p.version = Some(2);
    let name = p.name.clone();
    commit(s, p);
    format!("{} — {}", tr("Theme added"), name)
}

pub fn undo(s: &mut State) -> bool {
    match s.studio.undo() {
        Some(p) => {
            commit_restored(s, p);
            true
        }
        None => false,
    }
}

pub fn redo(s: &mut State) -> bool {
    match s.studio.redo() {
        Some(p) => {
            commit_restored(s, p);
            true
        }
        None => false,
    }
}

fn commit_restored(s: &mut State, p: ThemePack) {
    let id = format!("pack:{}", p.id);
    s.studio.store(p);
    refresh_packs(s);
    apply_theme(s, &id);
}

/// The theme dropdowns' names.
pub fn theme_names(s: &State) -> Vec<String> {
    all_themes(s).into_iter().map(|t| t.name).collect()
}

pub fn theme_index(s: &State, id: &str) -> usize {
    all_themes(s).iter().position(|t| t.id == id).unwrap_or(0)
}

/// Picks the dark- or light-mode theme from a dropdown.
pub fn set_mode_theme(s: &mut State, light: bool, i: usize) {
    let Some(id) = all_themes(s).get(i).map(|t| t.id.clone()) else {
        return;
    };
    if light == s.light_mode() {
        apply_theme(s, &id);
    } else if light {
        s.settings.appearance.theme_light = id;
    } else {
        s.settings.appearance.theme = id;
    }
}

/// Dark · Light · Follow Windows.
pub fn mode_ctl() -> Ctl {
    Ctl::Choice(
        |_| {
            ["Dark", "Light", "Follow Windows"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        },
        |s| match s.settings.appearance.theme_mode {
            crate::settings::ThemeMode::Dark => 0,
            crate::settings::ThemeMode::Light => 1,
            crate::settings::ThemeMode::System => 2,
        },
        |s, v| {
            s.settings.appearance.theme_mode = match v {
                1 => crate::settings::ThemeMode::Light,
                2 => crate::settings::ThemeMode::System,
                _ => crate::settings::ThemeMode::Dark,
            };
            s.apply_settings();
        },
    )
}

// ---------------------------------------------------------------- colour rows

fn col(get: impl Fn(&ThemePack) -> Option<String>, s: &State, fallback: Color) -> Color {
    get(&current(s))
        .and_then(|h| Color::parse(&h))
        .unwrap_or(fallback)
}

fn theme_now(s: &State) -> Theme {
    current(s).to_theme().unwrap_or_default()
}

macro_rules! accent_row {
    ($label:expr, $hint:expr, $i:expr) => {
        it(
            $label,
            $hint,
            Ctl::Colour(
                |s| theme_now(s).accent[$i],
                |s, c| edit(s, |p| p.accent[$i] = hex(c)),
            ),
        )
    };
}

/// The contrast report shown under the colours.
fn contrast_text(s: &State) -> String {
    let t = theme_now(s);
    let (i, m) = studio::contrast(&t);
    if i >= studio::AA_TEXT && m >= studio::AA_SECONDARY {
        format!(
            "AA ✓  {} {i:.1}:1 · {} {m:.1}:1",
            tr("text"),
            tr("secondary")
        )
    } else {
        format!(
            "⚠ {} ({i:.1}:1 · {m:.1}:1)",
            tr("Low contrast — text may be hard to read")
        )
    }
}

fn low_contrast(s: &State) -> bool {
    !studio::contrast_ok(&theme_now(s))
}

pub fn section() -> Section {
    Section {
        name: "Theme Studio",
        icon: Icon::Wand,
        tint: TINT_PRAYER,
        group: Group::Personalise,
        desc: "Make the notch yours: colours, glass, shape and sharing",
        reset: None,
        preview: true,
        items: vec![
            card("Themes"),
            it("Theme gallery", "", Ctl::Themes),
            it("Mode", "Light mode uses its own theme", mode_ctl()),
            it(
                "Editing",
                "",
                Ctl::Info(|s| {
                    let p = current(s);
                    if editable(s) {
                        format!("{} · {}", p.name, tr("your theme"))
                    } else {
                        format!("{} · {}", p.name, tr("built in — edits make a copy"))
                    }
                }),
            ),
            it(
                "Name",
                "",
                Ctl::Text(
                    |s| current(s).name,
                    |s, v| {
                        let v: String = v.trim().chars().take(40).collect();
                        if !v.is_empty() && v != current(s).name {
                            edit(s, |p| p.name = v);
                        }
                    },
                ),
            ),
            it(
                "New theme",
                "Starts from the one you're using",
                Ctl::Button("New", |s| {
                    let mut p = current(s);
                    let names: Vec<String> = s.studio.user.iter().map(|u| u.name.clone()).collect();
                    p.name = copy_name(&tr("My theme"), &names).replace(" copy", "");
                    p.id = unique_id(&p.name, &taken(s));
                    p.version = Some(2);
                    let n = p.name.clone();
                    commit(s, p);
                    format!("{} — {n}", tr("Theme created"))
                }),
            ),
            it(
                "Duplicate",
                "",
                Ctl::Button("Duplicate", |s| {
                    let mut p = current(s);
                    let names: Vec<String> = s.studio.user.iter().map(|u| u.name.clone()).collect();
                    p.name = copy_name(&p.name, &names);
                    p.id = unique_id(&p.name, &taken(s));
                    p.version = Some(2);
                    let n = p.name.clone();
                    commit(s, p);
                    format!("{} — {n}", tr("Duplicated"))
                }),
            ),
            when(
                it(
                    "Delete theme",
                    "Removes its file; Midnight takes over",
                    Ctl::Confirm("Delete", |s| {
                        let p = current(s);
                        s.studio.delete(&p.id);
                        refresh_packs(s);
                        apply_theme(s, "midnight");
                        format!("{} — {}", tr("Theme deleted"), p.name)
                    }),
                ),
                editable,
            ),
            card("Colours"),
            accent_row!("Accent start", "The gradient's first stop", 0),
            accent_row!("Accent middle", "", 1),
            accent_row!("Accent end", "", 2),
            it(
                "Surface",
                "Tiles and cards",
                Ctl::Colour(
                    |s| theme_now(s).surface,
                    |s, c| edit(s, |p| p.surface = hex(c)),
                ),
            ),
            it(
                "Raised surface",
                "Buttons and fields",
                Ctl::Colour(
                    |s| theme_now(s).surface_raised,
                    |s, c| edit(s, |p| p.surface_raised = Some(hex(c))),
                ),
            ),
            it(
                "Text",
                "",
                Ctl::Colour(
                    |s| col(|p| p.ink.clone(), s, INK),
                    |s, c| edit(s, |p| p.ink = Some(hex(c))),
                ),
            ),
            it(
                "Secondary text",
                "",
                Ctl::Colour(
                    |s| col(|p| p.mist.clone(), s, MIST.with_alpha(1.0)),
                    |s, c| edit(s, |p| p.mist = Some(hex(c))),
                ),
            ),
            it(
                "Generate from one colour",
                "Pick one colour; the rest follow (tweak them after)",
                Ctl::Colour(
                    |s| theme_now(s).accent[1],
                    |s, c| edit(s, |p| studio::from_one_colour(p, c)),
                ),
            ),
            it("Contrast", "", Ctl::Info(contrast_text)),
            when(
                it(
                    "Fix contrast",
                    "Nudges text and glass until it reads well",
                    Ctl::Button("Fix", |s| {
                        let mut ok = true;
                        edit(s, |p| ok = studio::fix_contrast(p));
                        tr(if ok {
                            "Contrast fixed"
                        } else {
                            "Couldn't fix it fully; try a darker surface"
                        })
                    }),
                ),
                low_contrast,
            ),
            card("Glass"),
            it(
                "Glass tint",
                "The colour behind content",
                Ctl::Colour(
                    |s| theme_now(s).glass.tint,
                    |s, c| {
                        edit(s, |p| {
                            p.glass.get_or_insert_with(Default::default).tint = Some(hex(c))
                        })
                    },
                ),
            ),
            it(
                "Rim light",
                "The lit edge",
                Ctl::Colour(
                    |s| theme_now(s).glass.rim,
                    |s, c| {
                        edit(s, |p| {
                            p.glass.get_or_insert_with(Default::default).rim = Some(hex(c))
                        })
                    },
                ),
            ),
            it(
                "Opacity",
                "The theme's default glass strength (the Appearance slider still overrides)",
                Ctl::Slider {
                    min: 0.0,
                    max: 100.0,
                    step: 1.0,
                    get: |s| {
                        theme_now(s)
                            .glass_strength
                            .unwrap_or(s.settings.appearance.glass.strength)
                            * 100.0
                    },
                    set: |s, v| {
                        edit(s, |p| {
                            p.glass.get_or_insert_with(Default::default).strength = Some(v / 100.0)
                        })
                    },
                    fmt: |v| format!("{v:.0}%"),
                },
            ),
            it(
                "Blur",
                "The theme's default blur",
                Ctl::Slider {
                    min: 0.0,
                    max: 100.0,
                    step: 1.0,
                    get: |s| {
                        theme_now(s)
                            .glass_blur
                            .unwrap_or(s.settings.appearance.glass.blur)
                            * 100.0
                    },
                    set: |s, v| {
                        edit(s, |p| {
                            p.glass.get_or_insert_with(Default::default).blur = Some(v / 100.0)
                        })
                    },
                    fmt: |v| format!("{v:.0}%"),
                },
            ),
            card("Shape and type"),
            it(
                "Corner roundness",
                "Rounder or squarer corners everywhere",
                Ctl::Slider {
                    min: 50.0,
                    max: 150.0,
                    step: 5.0,
                    get: |s| theme_now(s).radius_scale * 100.0,
                    set: |s, v| edit(s, |p| p.radius_scale = Some(v / 100.0)),
                    fmt: |v| format!("{v:.0}%"),
                },
            ),
            it(
                "Font",
                "A family installed on this PC; empty = Segoe UI Variable",
                Ctl::Text(
                    |s| current(s).font.unwrap_or_default(),
                    |s, v| {
                        let v = v.trim().to_string();
                        if current(s).font.unwrap_or_default() != v {
                            edit(s, |p| p.font = (!v.is_empty()).then_some(v));
                        }
                    },
                ),
            ),
            card("Share"),
            it(
                "Export theme",
                "Save it as a .json file",
                Ctl::Button("Export…", |s| {
                    let p = current(s);
                    match crate::platform_dialogs::save_file(&format!("{}.json", p.id)) {
                        Some(path) => match serde_json::to_string_pretty(&p)
                            .map_err(|e| e.to_string())
                            .and_then(|t| std::fs::write(&path, t).map_err(|e| e.to_string()))
                        {
                            Ok(()) => tr("Theme exported"),
                            Err(e) => e,
                        },
                        None => String::new(),
                    }
                }),
            ),
            it(
                "Import theme",
                "A .json theme file",
                Ctl::Button("Import…", |s| {
                    match crate::platform_dialogs::open_file("Theme", "*.json") {
                        Some(path) => match studio::read_pack_file(&path) {
                            Ok(p) => add_pack(s, p),
                            Err(e) => format!("{}: {e}", tr("Not a valid theme")),
                        },
                        None => String::new(),
                    }
                }),
            ),
            it(
                "Share code",
                "A short text code anyone can paste into Notchly",
                Ctl::Button("Copy code", |s| {
                    let code = studio::share_code(&current(s));
                    s.requests.push(crate::app::Request::CopyText(code));
                    tr("Share code copied")
                }),
            ),
            it(
                "Paste a share code",
                "Copy a code (NTH2:…), then paste it here",
                Ctl::Button("Paste", |s| {
                    let clip = crate::platform::clipboard::read(Default::default(), &[]);
                    match clip {
                        Some(crate::platform::clipboard::Clip::Text(t)) => {
                            match studio::from_share_code(&t) {
                                Ok(p) => add_pack(s, p),
                                Err(e) => e,
                            }
                        }
                        _ => tr("Copy a theme code first"),
                    }
                }),
            ),
            it(
                "Themes folder",
                "Your theme files",
                Ctl::Button("Open", |_| {
                    crate::platform::shell_open(&crate::theme::themes_dir().to_string_lossy());
                    String::new()
                }),
            ),
        ],
    }
}
