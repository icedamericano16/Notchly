//! The --demo script: cycles through activities, pop-ups and looks for screenshots and QA.

use super::*;

impl State {
    pub(super) fn tick_demo(&mut self, start: Instant, step: usize, now: Instant) {
        let due = start + Duration::from_millis(2600 * step as u64);
        if now < due {
            return;
        }
        self.demo = Some((start, step + 1));
        // after the tour, a glass showcase: presets, glass-first themes and both tiers;
        // the user's appearance comes back afterwards and nothing is saved
        // the cycle: tour (0-11), glass showcase (12-19), P0 rectangle check (20-38)
        let cycle = step % 39;
        if cycle >= 20 {
            self.demo_p0(cycle - 20);
            return;
        }
        let glass_step = cycle;
        if glass_step >= 12 {
            use crate::settings::{GlassPreset, Material};
            if glass_step == 12 {
                self.demo_appearance = Some(self.settings.appearance.clone());
                self.settings.appearance.material = Material::LiquidGlass;
                self.open(PageId::Dashboard);
            }
            let a = &mut self.settings.appearance;
            match glass_step {
                12 => a.glass.apply_preset(GlassPreset::Crystal),
                13 => a.glass.apply_preset(GlassPreset::Default),
                14 => a.glass.apply_preset(GlassPreset::Frosted),
                15 => a.glass.apply_preset(GlassPreset::Tinted),
                16 => {
                    a.glass.apply_preset(GlassPreset::Obsidian);
                    a.theme = "obsidian".into();
                }
                17 => {
                    a.glass.apply_preset(GlassPreset::Default);
                    a.theme = "sapphire".into();
                    self.open(PageId::Timer);
                }
                18 => {
                    a.material = Material::Glass;
                    a.theme = "aurora_glass".into();
                }
                _ => {
                    if let Some(orig) = self.demo_appearance.take() {
                        self.settings.appearance = orig;
                    }
                    self.collapse();
                }
            }
            self.apply_settings();
            return;
        }
        match cycle {
            0 => {
                self.popup(Popup::new(PopKind::Volume, Icon::Volume2, "Volume", "48%").value(0.48))
            }
            1 => self.popup(
                Popup::new(PopKind::Brightness, Icon::Brightness, "Brightness", "70%").value(0.7),
            ),
            2 => self.popup(
                Popup::new(PopKind::Caps, Icon::Caps, "Caps Lock on", "").tint(self.accent()[0]),
            ),
            3 => self.popup(
                Popup::new(
                    PopKind::Language,
                    Icon::Keyboard,
                    "Nepali (Nepal)",
                    "Input language",
                )
                .badge("NE"),
            ),
            4 => self.popup(
                Popup::new(
                    PopKind::Battery,
                    Icon::Bolt,
                    "Charging",
                    "64% · 48 min to full",
                )
                .value(0.64)
                .tint(GOOD),
            ),
            5 => self.popup(Popup::new(
                PopKind::Network,
                Icon::Wifi3,
                "Home Wi-Fi",
                "Connected",
            )),
            6 => self.popup(Popup::new(
                PopKind::AudioDevice,
                Icon::Headphones,
                "Headphones",
                "Now playing through",
            )),
            7 => {
                self.timer_cmd(TimerCmd::Preset(1));
            }
            8 => self.open(PageId::Dashboard),
            9 => self.open(PageId::Timer),
            10 => self.collapse(),
            _ => {
                let mut p = Popup::new(
                    PopKind::Notification,
                    Icon::Bell,
                    "Demo notification",
                    "This is how a mirrored toast looks in Notchly.",
                )
                .duration_ms(3000);
                p.source = "Demo".into();
                self.popup(p);
            }
        }
    }

    /// `--demo` P0 check: every material x style x view, so a halo or box around the notch
    /// would show up somewhere (k = 0..18; 18 restores the user's look).
    fn demo_p0(&mut self, k: usize) {
        use crate::settings::{Material, Style};
        if k == 0 {
            self.demo_appearance = Some(self.settings.appearance.clone());
        }
        if k >= 18 {
            if let Some(orig) = self.demo_appearance.take() {
                self.settings.appearance = orig;
            }
            self.collapse();
            self.apply_settings();
            return;
        }
        let a = &mut self.settings.appearance;
        a.material = [Material::Solid, Material::Glass, Material::LiquidGlass][k / 6];
        a.style = [Style::Attached, Style::Floating][(k / 3) % 2];
        self.apply_settings();
        match k % 3 {
            0 => {
                self.collapse();
                self.popup(Popup::new(PopKind::Volume, Icon::Volume2, "Volume", "48%").value(0.48));
            }
            1 => self.open(PageId::Dashboard),
            _ => self.open(PageId::Notes),
        }
    }
}
