//! Drawing the Settings window: sidebar, header, cards, rows and their controls, search results, the live preview, the colour picker and toasts.

use super::*;

impl SettingsWin {
    pub(super) fn draw_sidebar(&mut self, st: &State, cv: &mut Canvas, h: f32) {
        let side = Rect::new(0.0, 0.0, SET_SIDEBAR_W, h);
        cv.rect(side, NOTCH.with_alpha(SET_SIDEBAR_ALPHA));
        cv.rect(Rect::new(side.right() - 1.0, 0.0, 1.0, h), LINE);
        // app mark: a little notch on an accent squircle
        let mark = Rect::new(18.0, 18.0, 26.0, 26.0);
        if let Some(b) = cv.accent_brush(mark.x, mark.right(), 1.0) {
            cv.rrect_brush(mark, 8.0, crate::render::canvas::as_brush(&b));
        }
        cv.rrect(
            Rect::centered(mark.cx(), mark.y + 7.0, 14.0, 5.0),
            2.5,
            NOTCH,
        );
        cv.text(
            "Notchly",
            &text::SET_CARD.size(SET_APP_TITLE),
            Rect::new(mark.right() + 10.0, mark.y, 140.0, mark.h),
            cv.theme.ink,
            Align::Left,
        );
        let sr = Rect::new(14.0, 58.0, side.w - 28.0, 32.0);
        self.field(cv, SEARCH_ID, sr, "Search settings", Some(Icon::Search));
        let searching = !self.query().is_empty();

        // ---- grouped sections (scrolls when the window is short)
        let nav = Rect::new(
            0.0,
            sr.bottom() + 10.0,
            side.w,
            h - sr.bottom() - 10.0 - 44.0,
        );
        cv.push_clip(nav);
        let mut y = nav.y + 4.0 - self.nav_scroll.value;
        let mut rows: Vec<(usize, Rect)> = Vec::new();
        for g in Group::ALL {
            cv.text(
                &crate::i18n::tr(g.name()),
                &text::SET_GROUP,
                Rect::new(22.0, y + 8.0, side.w - 40.0, 14.0),
                FAINT,
                Align::Left,
            );
            y += SET_GROUP_H;
            for (i, _) in self
                .sections
                .iter()
                .enumerate()
                .filter(|(_, s)| s.group == g)
            {
                rows.push((i, Rect::new(10.0, y, side.w - 20.0, SET_NAV_ROW_H)));
                y += SET_NAV_ROW_H + 2.0;
            }
            y += 6.0;
        }
        let nav_h = y + self.nav_scroll.value - nav.y;
        let max = (nav_h - nav.h).max(0.0);
        if self.nav_scroll.target > max {
            self.nav_scroll.set(max);
        }
        // the selected pill glides to its row (position snappy, height gentle, a little
        // stretch while it travels)
        if let Some((_, r)) = rows.iter().find(|(i, _)| *i == self.section) {
            if !self.pill_placed {
                self.pill_y.snap(r.y + self.nav_scroll.value);
                self.pill_placed = true;
            }
            self.pill_y.set(r.y + self.nav_scroll.value);
            let stretch = (self.pill_y.velocity.abs() * SET_PILL_STRETCH).min(SET_PILL_STRETCH_MAX);
            self.pill_h.set(r.h + stretch);
            if !searching {
                let ph = self.pill_h.value;
                let pr = Rect::new(
                    r.x,
                    self.pill_y.value - self.nav_scroll.value + (r.h - ph) / 2.0,
                    r.w,
                    ph,
                );
                if let Some(b) = cv.accent_brush(pr.x, pr.right(), SET_PILL_ALPHA) {
                    cv.rrect_brush(pr, 10.0, crate::render::canvas::as_brush(&b));
                }
            }
        }
        for (i, r) in rows {
            let s = &self.sections[i];
            let active = i == self.section && !searching;
            if !active && cv.hovered(r) && cv.hovered(nav) {
                cv.rrect(r, 10.0, cv.theme.ink.with_alpha(SET_HOVER_LIFT));
            }
            // icon in its feature tint inside a small glass squircle
            let ib = Rect::centered(r.x + 21.0, r.cy(), SET_NAV_ICON_BOX, SET_NAV_ICON_BOX);
            cv.rrect(ib, 8.0, s.tint.with_alpha(if active { 0.22 } else { 0.14 }));
            cv.stroke_rrect(ib.inset(0.5), 7.5, WHITE.with_alpha(0.08), 1.0);
            cv.icon(s.icon, ib.cx(), ib.cy(), SET_NAV_ICON, s.tint);
            cv.text(
                &crate::i18n::tr(s.name),
                if active { &text::BODY_MED } else { &text::BODY },
                Rect::new(r.x + 42.0, r.y, r.w - 46.0, r.h),
                cv.theme.ink,
                Align::Left,
            );
            cv.zone_named(r, Action::Settings(NAV, i as u16), true, s.name, 10.0);
        }
        cv.pop_clip();

        // ---- "Saved" check
        if let Some(t) = self.saved_at {
            let e = t.elapsed().as_secs_f32() * 1000.0;
            let a = 1.0 - ((e - SAVED_HOLD_MS) / SET_FADE_MS).clamp(0.0, 1.0);
            if a > 0.0 {
                let s = self.saved_pop.value.max(0.0);
                let c = Rect::new(18.0, h - 34.0, 120.0, 20.0);
                cv.push_alpha(a);
                cv.push_tf(0.0, 0.0, s, c.x + 9.0, c.cy());
                cv.circle(c.x + 9.0, c.cy(), 9.0, GOOD.with_alpha(0.18));
                cv.icon(Icon::Check, c.x + 9.0, c.cy(), 11.0, GOOD);
                cv.pop_tf();
                cv.text(
                    &crate::i18n::tr("Saved"),
                    &text::CAPTION.weight(600),
                    Rect::new(c.x + 24.0, c.y, 90.0, c.h),
                    cv.theme.mist,
                    Align::Left,
                );
                cv.pop_alpha();
            }
        }
        let _ = st;
    }

    pub(super) fn draw_header(&mut self, st: &State, cv: &mut Canvas, r: Rect, searching: bool) {
        let sec = &self.sections[self.section];
        let (title, desc) = if searching {
            (crate::i18n::tr("Search results"), String::new())
        } else {
            (crate::i18n::tr(sec.name), crate::i18n::tr(sec.desc))
        };
        // buttons on the right first, so the title knows its room
        let by = r.y + 18.0;
        let mut right = r.right();
        if !searching {
            if sec.reset.is_some() {
                let armed = self.confirm.is_some_and(|c| c.0 == RESET_SECTION);
                if armed {
                    // the button morphs into "Reset · Cancel", sliding in from the right
                    let e = self
                        .confirm
                        .map(|c| c.1.elapsed().as_secs_f32())
                        .unwrap_or(1.0);
                    let k = if st.env.reduced {
                        1.0
                    } else {
                        ease_out(e / 0.18)
                    };
                    let dx = (1.0 - k) * 16.0;
                    cv.push_alpha(k);
                    let cw = cv.pill_width("Cancel", 76.0);
                    let cr = Rect::new(right - cw + dx, by, cw, 28.0);
                    cv.pill_button("Cancel", cr, Action::Settings(CMD, CMD_RESET_CANCEL), false);
                    let rw = cv.pill_width("Reset", 72.0);
                    let rr = Rect::new(cr.x - 8.0 - rw, by, rw, 28.0);
                    danger_button(cv, "Reset", rr, Action::Settings(CMD, CMD_RESET_OK));
                    cv.pop_alpha();
                    right = rr.x - 10.0;
                } else {
                    let rw = cv.pill_width("Reset section", 110.0);
                    let rr = Rect::new(right - rw, by, rw, 28.0);
                    cv.pill_button("Reset section", rr, Action::Settings(CMD, CMD_RESET), false);
                    right = rr.x - 10.0;
                }
            }
            // Theme Studio: redo too (its own history)
            let studio = self.in_studio();
            if studio {
                let rr = Rect::new(right - 32.0, by - 2.0, 32.0, 32.0);
                let can = st.studio.can_redo();
                cv.push_alpha(if can { 1.0 } else { DISABLED_ALPHA });
                let hv = can && cv.hovered(rr);
                cv.circle(
                    rr.cx(),
                    rr.cy(),
                    15.0,
                    LINE.with_alpha(if hv { 1.8 } else { 1.0 }),
                );
                cv.icon(Icon::Redo, rr.cx(), rr.cy(), 15.0, cv.theme.ink);
                cv.pop_alpha();
                if can {
                    cv.zone_named(rr, Action::Settings(CMD, CMD_REDO), true, "Redo", 16.0);
                }
                right = rr.x - 8.0;
            }
            // Undo (Ctrl+Z), enabled after a change
            let ur = Rect::new(right - 32.0, by - 2.0, 32.0, 32.0);
            let can = if studio {
                st.studio.can_undo()
            } else {
                !self.undo.is_empty()
            };
            cv.push_alpha(if can { 1.0 } else { DISABLED_ALPHA });
            let hv = can && cv.hovered(ur);
            cv.circle(
                ur.cx(),
                ur.cy(),
                15.0,
                LINE.with_alpha(if hv { 1.8 } else { 1.0 }),
            );
            cv.icon(Icon::Undo, ur.cx(), ur.cy(), 15.0, cv.theme.ink);
            cv.pop_alpha();
            if can {
                cv.zone_named(ur, Action::Settings(CMD, CMD_UNDO), true, "Undo", 16.0);
            }
            right = ur.x - 12.0;
        }
        cv.text(
            &title,
            &text::SET_TITLE,
            Rect::new(r.x, r.y + 6.0, right - r.x, 32.0),
            cv.theme.ink,
            Align::Left,
        );
        if !desc.is_empty() {
            cv.text(
                &desc,
                &text::SET_DESC,
                Rect::new(r.x, r.y + 38.0, right - r.x, 18.0),
                cv.theme.mist,
                Align::Left,
            );
        }
    }

    pub(super) fn draw_cards(&mut self, st: &State, cv: &mut Canvas, area: Rect) -> f32 {
        let cards = self.cards(st);
        let mut y = area.y - self.scroll.value;
        for (ci, card) in cards.iter().enumerate() {
            let title_h = if card.title.is_some() {
                SET_CARD_TITLE_H
            } else {
                6.0
            };
            let rows_h: f32 = card.ids.iter().map(|id| self.row_height(*id)).sum();
            let r = Rect::new(area.x, y, area.w, title_h + rows_h + 6.0);
            y = r.bottom() + SET_CARD_GAP;
            if let Some(target) = self.scroll_to {
                if let Some(pos) = card.ids.iter().position(|i| *i == target) {
                    let row_y = r.y
                        + title_h
                        + card.ids[..pos]
                            .iter()
                            .map(|id| self.row_height(*id))
                            .sum::<f32>();
                    let want = self.scroll.target + (row_y - area.y) - 40.0;
                    self.scroll.set(want.max(0.0));
                    self.scroll_to = None;
                }
            }
            if !cv.visible(r) {
                continue;
            }
            let (a, dy) = self.card_in(st, ci);
            if a <= 0.0 {
                continue;
            }
            cv.push_alpha(a);
            cv.push_tf(0.0, dy, 1.0, r.cx(), r.cy());
            cv.sq(
                r,
                SET_CARD_R,
                cv.theme.surface_raised.with_alpha(SET_CARD_ALPHA),
            );
            cv.stroke_rrect(r.inset(0.5), SET_CARD_R - 0.5, LINE, 1.0);
            if let Some(t) = card.title {
                cv.text(
                    &crate::i18n::tr(t),
                    &text::SET_CARD,
                    Rect::new(
                        r.x + SET_CARD_PAD,
                        r.y + 12.0,
                        r.w - 2.0 * SET_CARD_PAD,
                        22.0,
                    ),
                    cv.theme.ink,
                    Align::Left,
                );
            }
            let mut ry = r.y + title_h;
            for (k, id) in card.ids.iter().enumerate() {
                let rh = self.row_height(*id);
                let row = Rect::new(r.x, ry, r.w, rh);
                if k > 0 {
                    cv.rect(
                        Rect::new(r.x + SET_CARD_PAD, ry, r.w - 2.0 * SET_CARD_PAD, 1.0),
                        LINE,
                    );
                }
                if cv.visible(row) {
                    self.draw_item(st, cv, *id, row);
                    self.draw_flash(cv, *id, row);
                }
                ry += rh;
            }
            cv.pop_tf();
            cv.pop_alpha();
        }
        y + self.scroll.value - area.y
    }

    pub(super) fn draw_flash(&self, cv: &mut Canvas, id: u16, row: Rect) {
        if let Some((fid, t)) = self.flash {
            let e = t.elapsed().as_secs_f32() * 1000.0;
            if fid == id && e < SEARCH_FLASH_MS {
                let a = 1.0 - e / SEARCH_FLASH_MS;
                cv.stroke_rrect(
                    row.inset_xy(6.0, 3.0),
                    12.0,
                    cv.theme.accent[0].with_alpha(a),
                    2.0,
                );
                cv.rrect(
                    row.inset_xy(6.0, 3.0),
                    12.0,
                    cv.theme.accent[0].with_alpha(0.08 * a),
                );
            }
        }
    }

    pub(super) fn draw_results(&mut self, st: &State, cv: &mut Canvas, area: Rect) -> f32 {
        let ids = self.results(st);
        let y0 = area.y - self.scroll.value;
        if ids.is_empty() {
            cv.text(
                &crate::i18n::tr("No settings match"),
                &text::BODY,
                Rect::new(area.x, y0 + 8.0, area.w, 20.0),
                cv.theme.mist,
                Align::Left,
            );
            return 40.0;
        }
        let count = format!("{} {}", ids.len(), crate::i18n::tr("results"));
        let card = Rect::new(
            area.x,
            y0,
            area.w,
            SET_CARD_TITLE_H + ids.len() as f32 * SET_ROW_H + 6.0,
        );
        cv.sq(
            card,
            SET_CARD_R,
            cv.theme.surface_raised.with_alpha(SET_CARD_ALPHA),
        );
        cv.stroke_rrect(card.inset(0.5), SET_CARD_R - 0.5, LINE, 1.0);
        cv.text(
            &count,
            &text::SET_CARD,
            Rect::new(card.x + SET_CARD_PAD, card.y + 12.0, card.w, 22.0),
            cv.theme.ink,
            Align::Left,
        );
        for (k, id) in ids.iter().enumerate() {
            let r = Rect::new(
                card.x,
                card.y + SET_CARD_TITLE_H + k as f32 * SET_ROW_H,
                card.w,
                SET_ROW_H,
            );
            if !cv.visible(r) {
                continue;
            }
            if k > 0 {
                cv.rect(
                    Rect::new(r.x + SET_CARD_PAD, r.y, r.w - 2.0 * SET_CARD_PAD, 1.0),
                    LINE,
                );
            }
            let Some(item) = self.item(*id).copied() else {
                continue;
            };
            let sec = &self.sections[*id as usize / 100];
            let crumb = match self.card_of(*id) {
                Some(c) => format!("{} › {}", crate::i18n::tr(sec.name), crate::i18n::tr(c)),
                None => crate::i18n::tr(sec.name),
            };
            let a = Action::Settings(RESULT, *id);
            if cv.hovered(r) || self.keyboard_focus.as_ref() == Some(&a) {
                cv.rrect(
                    r.inset_xy(6.0, 3.0),
                    12.0,
                    cv.theme.ink.with_alpha(SET_HOVER_LIFT),
                );
            }
            let ib = Rect::centered(r.x + SET_CARD_PAD + 13.0, r.cy(), 26.0, 26.0);
            cv.rrect(ib, 8.0, sec.tint.with_alpha(0.14));
            cv.icon(sec.icon, ib.cx(), ib.cy(), 15.0, sec.tint);
            let tx = ib.right() + 12.0;
            cv.text(
                &crate::i18n::tr(item.label),
                &text::SET_LABEL,
                Rect::new(tx, r.y + 8.0, r.w - tx + r.x - 40.0, 18.0),
                cv.theme.ink,
                Align::Left,
            );
            cv.text(
                &crumb,
                &text::SET_DESC,
                Rect::new(tx, r.y + 27.0, r.w - tx + r.x - 40.0, 16.0),
                cv.theme.mist,
                Align::Left,
            );
            cv.icon(Icon::ChevronRight, r.right() - 24.0, r.cy(), 13.0, FAINT);
            cv.zone_named(r.inset_xy(6.0, 3.0), a, true, item.label, 12.0);
        }
        card.bottom() + self.scroll.value - area.y
    }

    pub(super) fn draw_dropdown(&mut self, st: &State, cv: &mut Canvas) {
        let Some((id, b)) = self.dropdown else { return };
        let Some(Ctl::Choice(options, get, _)) = self.item(id).map(|i| i.ctl) else {
            self.dropdown = None;
            return;
        };
        let o = options(st);
        let cur = get(st);
        let rh = 30.0;
        let h = o.len() as f32 * rh + 8.0;
        let below = b.bottom() + 6.0 + h < self.size.1 - 8.0;
        let r = Rect::new(
            b.x,
            if below {
                b.bottom() + 6.0
            } else {
                b.y - 6.0 - h
            },
            b.w.max(180.0),
            h,
        );
        cv.sq(r.inset(-1.0), 13.0, NOTCH.with_alpha(0.35));
        cv.sq(r, 12.0, cv.theme.surface_raised);
        cv.stroke_rrect(r.inset(0.5), 11.5, LINE.with_alpha(1.6), 1.0);
        for (i, s) in o.iter().enumerate() {
            let rr = Rect::new(r.x + 4.0, r.y + 4.0 + i as f32 * rh, r.w - 8.0, rh);
            let a = Action::Settings(DROP, i as u16);
            if cv.hovered(rr) || self.keyboard_focus.as_ref() == Some(&a) {
                cv.rrect(rr, 8.0, cv.theme.ink.with_alpha(0.08));
            }
            if i == cur {
                cv.icon(Icon::Check, rr.x + 14.0, rr.cy(), 12.0, cv.theme.accent[0]);
            }
            cv.text(
                s,
                &text::BODY,
                Rect::new(rr.x + 28.0, rr.y, rr.w - 34.0, rr.h),
                cv.theme.ink,
                Align::Left,
            );
            cv.zone_named(rr, a, true, s, 8.0);
        }
    }

    pub(super) fn draw_toast(&self, cv: &mut Canvas, w: f32, h: f32) {
        let Some((m, t)) = &self.toast else { return };
        let e = t.elapsed().as_secs_f32() * 1000.0;
        if m.is_empty() || e > TOAST_MS + SET_FADE_MS {
            return;
        }
        let k_in = ease_out(e / SET_FADE_MS);
        let k_out = 1.0 - ((e - TOAST_MS) / SET_FADE_MS).clamp(0.0, 1.0);
        let a = k_in.min(k_out);
        let tw = cv.measure(m, &text::BODY_MED) + 40.0;
        let cx = SET_SIDEBAR_W + (w - SET_SIDEBAR_W) / 2.0;
        let r = Rect::new(
            cx - tw / 2.0,
            h - 52.0 + (1.0 - k_in) * TOAST_RISE,
            tw,
            34.0,
        );
        cv.push_alpha(a);
        cv.sq(r.inset(-1.0), 18.0, NOTCH.with_alpha(0.3));
        cv.sq(r, 17.0, cv.theme.surface_raised);
        cv.stroke_rrect(r.inset(0.5), 16.5, WHITE.with_alpha(0.1), 1.0);
        cv.text(m, &text::BODY_MED, r, cv.theme.ink, Align::Center);
        cv.pop_alpha();
    }

    /// The Fluid Style gallery: each card plays its style (half speed until hovered).
    pub(super) fn fluid_gallery(&self, st: &State, cv: &mut Canvas, id: u16, r: Rect) {
        use crate::render::glass::fluid;
        let cols = self.fluid_cols();
        let area = Rect::new(
            r.x + SET_CARD_PAD,
            r.y + 12.0,
            r.w - 2.0 * SET_CARD_PAD,
            r.h - 24.0,
        );
        let cw = (area.w - THEME_GAP * (cols as f32 - 1.0)) / cols as f32;
        let t = if st.env.reduced {
            0.0
        } else {
            self.started.elapsed().as_secs_f32()
        };
        let accent = st.accent();
        for (i, style) in fluid::STYLES.iter().enumerate() {
            let c = Rect::new(
                area.x + (i % cols) as f32 * (cw + THEME_GAP),
                area.y + (i / cols) as f32 * (FLUID_CARD_H + THEME_GAP),
                cw,
                FLUID_CARD_H,
            );
            if !cv.visible(c) {
                continue;
            }
            let a = Action::Settings(id, i as u16 + 1);
            let hover = cv.hovered(c);
            let (hv, pv) = cv.anim(&a, hover, false);
            cv.push_tf(0.0, 0.0, 1.0 + 0.02 * hv - 0.03 * pv, c.cx(), c.cy());
            cv.rrect(c, 14.0, st.theme.glass.tint.lerp(NOTCH, 0.3));
            let body = Rect::new(c.x + 10.0, c.y + 10.0, c.w - 20.0, c.h - 36.0);
            cv.push_clip(c.inset(1.0));
            let ctx = fluid::Ctx {
                t: t * if hover { 1.0 } else { 0.5 },
                rect: body,
                accent,
                tint: st.theme.glass.tint,
                intensity: 2.0,
                light: (0.4 * (t * 0.7).sin(), -0.3),
                kick: 0.0,
            };
            fluid::draw(cv, &fluid::marks(style, &ctx));
            if style.id == "ripple" {
                // Ripple reacts rather than drifts: show a ring spreading out
                let k = if st.env.reduced {
                    0.5
                } else {
                    (t * if hover { 0.8 } else { 0.4 }).fract()
                };
                cv.stroke_circle(
                    body.cx(),
                    body.cy(),
                    6.0 + k * body.w * 0.4,
                    WHITE.with_alpha(0.45 * (1.0 - k)),
                    1.5,
                );
            }
            cv.pop_clip();
            cv.stroke_rrect(c.inset(0.5), 13.5, WHITE.with_alpha(0.10 + 0.06 * hv), 1.0);
            cv.text(
                &crate::i18n::tr(style.name),
                &text::CAPTION.weight(600),
                Rect::new(c.x + 12.0, c.bottom() - 24.0, c.w - 24.0, 18.0),
                WHITE,
                Align::Left,
            );
            if st.settings.appearance.fluid.style == style.id {
                cv.stroke_rrect(c.inset(-2.5), 16.0, cv.theme.accent[0], 2.0);
                cv.circle(c.right() - 12.0, c.y + 12.0, 8.0, cv.theme.accent[0]);
                cv.icon(Icon::Check, c.right() - 12.0, c.y + 12.0, 10.0, WHITE);
            }
            cv.pop_tf();
            cv.zone_named(c, a, true, style.name, 14.0);
        }
    }

    pub(super) fn preview(&mut self, st: &State, cv: &mut Canvas, r: Rect) {
        use crate::render::glass::{compositor, lens::Body, Backdrop, Tier};
        let material = st.settings.appearance.material;
        let glassy = material != Material::Solid;
        let scale = self.dpi / 96.0;
        if self.backdrop == PreviewBackdrop::Desktop && self.desktop.is_none() {
            // decoded once, the first time it's picked (never saved anywhere)
            self.desktop = Some(
                crate::platform::wallpaper_path()
                    .and_then(|p| crate::render::image::decode_file(&p, 1600).ok()),
            );
        }
        let t = if st.env.reduced {
            0.0
        } else {
            self.started.elapsed().as_secs_f32() % PREVIEW_MOVING_SECS
        };
        let (pw, ph) = (
            (r.right() * scale).ceil() as u32,
            (r.bottom() * scale).ceil() as u32,
        );
        let wall = compositor::preview_backdrop(
            cv,
            pw,
            ph,
            self.backdrop,
            t,
            self.desktop.as_ref().and_then(|d| d.as_ref()),
        );
        if let Some(b) = &wall {
            if let Ok(g) = shapes::squircle(&cv.gfx.factory.clone(), r, SET_CARD_R) {
                use windows::core::Interface;
                if let Ok(g) = g.cast() {
                    cv.push_geo_clip_any(&g);
                    unsafe {
                        cv.ctx.DrawBitmap(
                            b,
                            Some(&windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                                left: 0.0,
                                top: 0.0,
                                right: pw as f32 / scale,
                                bottom: ph as f32 / scale,
                            }),
                            1.0,
                            windows::Win32::Graphics::Direct2D::D2D1_INTERPOLATION_MODE_LINEAR,
                            None,
                            None,
                        );
                    }
                    cv.pop_layer();
                }
            }
        } else {
            cv.sq(r, SET_CARD_R, NOTCH.lerp(SURFACE_RAISED, 0.35));
        }
        cv.stroke_rrect(r.inset(0.5), SET_CARD_R - 0.5, LINE, 1.0);
        let size = match st.settings.appearance.size {
            115 => 1.15,
            130 => 1.3,
            150 => 1.5,
            _ => 1.0,
        };
        let floating = st.settings.appearance.style == Style::Floating;
        let (iw, ih) = (self.preview_w.value * size, self.preview_h.value * size);
        let top = r.y + if floating { 10.0 } else { 0.0 };
        let radius = (ih * 0.45).min(26.0);
        let factory = cv.gfx.factory.clone();
        cv.push_clip(r);
        if let Ok(p) = shapes::notch_path(
            &factory,
            r.cx(),
            top,
            iw,
            ih,
            radius,
            if floating { 0.0 } else { 9.0 },
            floating,
        ) {
            match (&wall, glassy) {
                (Some(b), true) => {
                    use windows::core::Interface;
                    let tier = if material == Material::LiquidGlass {
                        Tier::Liquid
                    } else {
                        Tier::Frosted
                    };
                    let f = crate::app::glass::preview_frame(
                        st,
                        tier,
                        1.0,
                        Some(Backdrop {
                            bitmap: b.clone(),
                            slot: 1,
                            transform: None,
                        }),
                        (r.cx() - iw * 0.25, top + ih * 0.25),
                        floating,
                    );
                    let body = Body {
                        x: r.cx() - iw / 2.0,
                        y: top,
                        w: iw,
                        h: ih,
                        radius,
                        open_top: !floating,
                    };
                    if let Ok(g) = p.cast() {
                        compositor::surface(cv, &g, &[body], &f, floating);
                    }
                    let style =
                        crate::render::glass::fluid::style(&st.settings.appearance.fluid.style);
                    if style.animates() {
                        let mut c = st.fluid_ctx(Rect::new(r.cx() - iw / 2.0, top, iw, ih));
                        if !st.env.reduced && material == Material::LiquidGlass {
                            c.t = self.started.elapsed().as_secs_f32()
                                * st.settings.appearance.fluid.speed;
                        }
                        cv.push_geo_clip(&p);
                        crate::render::glass::fluid::draw(
                            cv,
                            &crate::render::glass::fluid::marks(style, &c),
                        );
                        cv.pop_layer();
                    }
                }
                _ => {
                    cv.fill_geo(&p, NOTCH);
                    if floating {
                        cv.stroke_geo(&p, RING, 1.0);
                    }
                }
            }
        }
        // a hint of content per state
        let ink = cv.theme.ink;
        let cy = top + ih / 2.0;
        match self.preview_phase {
            1 => {
                cv.circle(r.cx() - iw / 2.0 + 18.0, cy, 6.0, TINT_MUSIC);
                cv.progress(
                    Rect::new(r.cx() + iw / 2.0 - 60.0, cy - 1.5, 40.0, 3.0),
                    0.62,
                    true,
                );
            }
            2 => {
                cv.rrect(
                    Rect::new(r.cx() - iw / 2.0 + 16.0, cy - 14.0, 28.0, 28.0),
                    8.0,
                    TINT_WATER.with_alpha(0.85),
                );
                cv.rrect(
                    Rect::new(r.cx() - iw / 2.0 + 54.0, cy - 8.0, iw * 0.4, 6.0),
                    3.0,
                    ink.with_alpha(0.85),
                );
                cv.rrect(
                    Rect::new(r.cx() - iw / 2.0 + 54.0, cy + 3.0, iw * 0.28, 5.0),
                    2.5,
                    ink.with_alpha(0.45),
                );
            }
            3 => {
                for i in 0..3 {
                    let x = r.cx() - iw / 2.0 + 18.0 + i as f32 * ((iw - 36.0) / 3.0);
                    cv.rrect(
                        Rect::new(x, cy - 22.0, (iw - 36.0) / 3.0 - 8.0, 44.0),
                        10.0,
                        ink.with_alpha(0.10),
                    );
                }
            }
            _ => {}
        }
        cv.pop_clip();
        // ---- controls along the bottom: backdrop swatches · state segmented · play
        let by = r.bottom() - 38.0;
        let mut x = r.x + 14.0;
        for (i, k) in PreviewBackdrop::ALL.iter().enumerate() {
            let c = Rect::new(x, by + 3.0, 22.0, 22.0);
            let a = Action::Settings(CMD, CMD_BACKDROP + i as u16);
            let hv = cv.hovered(c);
            let s = if hv { SWATCH_HOVER } else { 1.0 };
            cv.push_tf(0.0, 0.0, s, c.cx(), c.cy());
            let sw = k.swatch();
            if let Some(g) = cv.gradient(
                crate::render::canvas::v2(c.x, c.y),
                crate::render::canvas::v2(c.right(), c.bottom()),
                &[(0.0, sw[0]), (1.0, sw[1])],
                1.0,
            ) {
                cv.rrect_brush(c, 11.0, crate::render::canvas::as_brush(&g));
            }
            cv.stroke_rrect(c.inset(0.5), 10.5, WHITE.with_alpha(0.35), 1.0);
            if *k == self.backdrop {
                cv.stroke_rrect(c.inset(-3.0), 14.0, WHITE, SWATCH_RING);
            }
            cv.pop_tf();
            cv.zone_named(c.inset(-3.0), a, true, k.name(), 14.0);
            x += 30.0;
        }
        let pw = 36.0;
        let play = Rect::new(r.right() - 14.0 - pw + 4.0, by, 28.0, 28.0);
        let sw = (r.w - (x - r.x) - pw - 40.0).clamp(120.0, 300.0);
        let seg = Rect::new(play.x - 10.0 - sw, by, sw, 28.0);
        cv.sq(seg, 14.0, NOTCH.with_alpha(0.45));
        let n = PREVIEW_STATES.len() as f32;
        let cw = seg.w / n;
        cv.rrect(
            Rect::new(
                seg.x + self.preview_phase as f32 * cw + 2.0,
                seg.y + 2.0,
                cw - 4.0,
                seg.h - 4.0,
            ),
            12.0,
            WHITE.with_alpha(0.18),
        );
        for (i, s) in PREVIEW_STATES.iter().enumerate() {
            let rr = Rect::new(seg.x + i as f32 * cw, seg.y, cw, seg.h);
            cv.text(
                &crate::i18n::tr(s),
                &text::CAPTION.weight(600),
                rr,
                if i == self.preview_phase {
                    WHITE
                } else {
                    WHITE.with_alpha(0.65)
                },
                Align::Center,
            );
            cv.zone_named(
                rr,
                Action::Settings(CMD, CMD_STATE + i as u16),
                true,
                s,
                12.0,
            );
        }
        cv.circle(play.cx(), play.cy(), 14.0, NOTCH.with_alpha(0.45));
        cv.icon(
            if self.preview_play {
                Icon::Pause
            } else {
                Icon::Play
            },
            play.cx(),
            play.cy(),
            12.0,
            WHITE,
        );
        cv.zone_named(
            play,
            Action::Settings(CMD, CMD_PLAY),
            true,
            if self.preview_play { "Pause" } else { "Play" },
            14.0,
        );
        cv.label(
            "Live preview",
            &text::MICRO,
            Rect::new(r.x + 14.0, r.y + 10.0, 200.0, 14.0),
            WHITE.with_alpha(0.7),
            Align::Left,
        );
    }

    pub(super) fn field(
        &self,
        cv: &mut Canvas,
        id: u16,
        r: Rect,
        placeholder: &str,
        icon: Option<Icon>,
    ) {
        let focused = self.focus == Some(id);
        cv.sq(r, R_INPUT, cv.theme.surface_raised);
        if focused {
            cv.stroke_rrect(
                r.inset(0.5),
                R_INPUT,
                cv.theme.accent[1].with_alpha(0.7),
                1.0,
            );
        }
        let mut x = r.x + 10.0;
        if let Some(ic) = icon {
            cv.icon(ic, x + 7.0, r.cy(), 14.0, cv.theme.mist);
            x += 22.0;
        }
        let f = self.fields.get(&id).cloned().unwrap_or_default();
        let tr = Rect::new(x, r.y, r.right() - x - 8.0, r.h);
        let multiline = r.h > 40.0;
        cv.push_clip(tr);
        if f.text.is_empty() {
            cv.text(
                placeholder,
                &text::BODY,
                if multiline {
                    Rect::new(tr.x, r.y + 6.0, tr.w, 18.0)
                } else {
                    tr
                },
                FAINT,
                Align::Left,
            );
        } else if multiline {
            cv.text_top(
                &f.text,
                &text::BODY.lines(5),
                tr.x,
                r.y + 6.0,
                tr.w,
                cv.theme.ink,
                Align::Left,
            );
        } else {
            let prefix: String = f.text.chars().take(f.caret).collect();
            let cx = cv.measure(&prefix, &text::BODY);
            let off = (cx - tr.w + 4.0).max(0.0);
            cv.text(
                &f.text,
                &text::BODY,
                Rect::new(tr.x - off, tr.y, tr.w + off + 400.0, tr.h),
                cv.theme.ink,
                Align::Left,
            );
            if focused {
                cv.rect(
                    Rect::new(tr.x + cx - off, r.cy() - 8.0, 1.5, 16.0),
                    cv.theme.accent[0],
                );
            }
        }
        cv.pop_clip();
        cv.zone_named(
            r,
            Action::Focus(InputId::Settings(id)),
            true,
            placeholder,
            R_INPUT,
        );
    }

    pub(super) fn draw_item(&mut self, st: &State, cv: &mut Canvas, id: u16, r: Rect) {
        let Some(item) = self.item(id).copied() else {
            return;
        };
        if matches!(item.ctl, Ctl::Themes) {
            self.gallery(st, cv, id, r);
            return;
        }
        if matches!(item.ctl, Ctl::Fluid) {
            self.fluid_gallery(st, cv, id, r);
            return;
        }
        let wide = matches!(item.ctl, Ctl::List(..) | Ctl::Chips(..));
        let label_w = if matches!(item.ctl, Ctl::Info(_)) {
            r.w * 0.34
        } else {
            r.w * 0.5 - SET_CARD_PAD
        };
        let lx = r.x + SET_CARD_PAD;
        let (ly, lh) = if wide {
            (r.y + 10.0, 18.0)
        } else if item.hint.is_empty() {
            (r.y, r.h)
        } else {
            (r.y + 8.0, 18.0)
        };
        cv.text(
            &crate::i18n::tr(item.label),
            &text::SET_LABEL,
            Rect::new(lx, ly, label_w, lh),
            cv.theme.ink,
            Align::Left,
        );
        if !item.hint.is_empty() {
            cv.text(
                &crate::i18n::tr(item.hint),
                &text::SET_DESC,
                Rect::new(lx, ly + 19.0, label_w, 16.0),
                cv.theme.mist,
                Align::Left,
            );
        }
        let cr = Rect::new(
            r.x + r.w * 0.5,
            r.y + 8.0,
            r.w * 0.5 - SET_CARD_PAD,
            r.h - 16.0,
        );
        match &item.ctl {
            Ctl::Card | Ctl::Themes | Ctl::Fluid => {}
            Ctl::Colour(get, _) => {
                let c = get(st);
                let open = self.picker.is_some_and(|p| p.0 == id);
                let b = Rect::new(cr.right() - 150.0, cr.cy() - 15.0, 150.0, 30.0);
                let hv = cv.hovered(b);
                cv.sq(
                    b,
                    15.0,
                    TRACK.with_alpha(if hv || open { 1.4 } else { 1.0 }),
                );
                cv.circle(b.x + 16.0, b.cy(), 10.0, c);
                cv.stroke_circle(b.x + 16.0, b.cy(), 10.0, WHITE.with_alpha(0.25), 1.0);
                cv.text(
                    &crate::theme::studio::hex(c),
                    &text::MONO,
                    Rect::new(b.x + 34.0, b.y, b.w - 40.0, b.h),
                    cv.theme.ink,
                    Align::Left,
                );
                cv.zone_named(b, Action::Settings(id, 0), true, item.label, 15.0);
            }
            Ctl::Toggle(get, _) => {
                let on = get(st);
                let t = Rect::new(
                    cr.right() - TOGGLE_W,
                    cr.cy() - TOGGLE_H / 2.0,
                    TOGGLE_W,
                    TOGGLE_H,
                );
                let k = self
                    .knobs
                    .entry(id)
                    .or_insert_with(|| Spring::unit(on as u8 as f32, SNAPPY));
                k.set(on as u8 as f32);
                let v = k.value.clamp(-0.1, 1.1);
                cv.rrect(t, TOGGLE_H / 2.0, TRACK);
                if v > 0.01 {
                    if let Some(b) = cv.accent_brush(t.x, t.right(), v.clamp(0.0, 1.0)) {
                        cv.rrect_brush(t, TOGGLE_H / 2.0, crate::render::canvas::as_brush(&b));
                    }
                }
                // the knob stretches mid-travel, then settles (liquid feel)
                let travel = 1.0 - (2.0 * v.clamp(0.0, 1.0) - 1.0).abs();
                let kw = TOGGLE_KNOB + TOGGLE_STRETCH * travel;
                let pad = (TOGGLE_H - TOGGLE_KNOB) / 2.0;
                let kx = t.x + pad + (t.w - 2.0 * pad - kw) * v.clamp(0.0, 1.0);
                let kr = Rect::new(kx, t.y + pad, kw, TOGGLE_KNOB);
                cv.rrect(
                    kr.inset(-0.5).translate(0.0, 0.8),
                    TOGGLE_KNOB / 2.0,
                    NOTCH.with_alpha(0.25),
                );
                cv.rrect(kr, TOGGLE_KNOB / 2.0, WHITE);
                cv.zone_named(
                    t.inset(-6.0),
                    Action::Settings(id, 0),
                    true,
                    item.label,
                    TOGGLE_H / 2.0 + 6.0,
                );
            }
            Ctl::Choice(options, get, _) => {
                let o = options(st);
                let cur = get(st).min(o.len().saturating_sub(1));
                let total: usize = o.iter().map(|s| crate::i18n::tr(s).chars().count()).sum();
                if o.len() <= 3 && total < 34 {
                    self.segmented(cv, id, item.label, &o, cur, cr);
                } else {
                    // a dropdown: shows the current value, opens a list
                    let dw = cr.w.min(260.0);
                    let d = Rect::new(cr.right() - dw, cr.cy() - 15.0, dw, 30.0);
                    let open = self.dropdown.is_some_and(|x| x.0 == id);
                    let hv = cv.hovered(d);
                    cv.sq(
                        d,
                        10.0,
                        TRACK.with_alpha(if hv || open { 1.4 } else { 1.0 }),
                    );
                    cv.text(
                        &o.get(cur).map(|s| crate::i18n::tr(s)).unwrap_or_default(),
                        &text::BODY,
                        Rect::new(d.x + 12.0, d.y, d.w - 36.0, d.h),
                        cv.theme.ink,
                        Align::Left,
                    );
                    cv.icon_rot(
                        Icon::ChevronDown,
                        d.right() - 14.0,
                        d.cy(),
                        12.0,
                        cv.theme.mist,
                        if open { std::f32::consts::PI } else { 0.0 },
                    );
                    cv.zone_named(d, Action::Settings(id, 0), true, item.label, 10.0);
                }
            }
            Ctl::Slider {
                min, max, get, fmt, ..
            } => {
                let v = get(st);
                let frac = ((v - min) / (max - min)).clamp(0.0, 1.0);
                let sr = Rect::new(cr.x, cr.y, cr.w - 76.0, cr.h);
                self.slider_ctl(
                    cv,
                    id,
                    item.label,
                    sr,
                    frac,
                    &fmt(v),
                    item.label == STRENGTH_LABEL,
                );
                cv.text(
                    &fmt(v),
                    &text::SET_VALUE,
                    Rect::new(cr.right() - 70.0, cr.y, 70.0, cr.h),
                    cv.theme.mist,
                    Align::Right,
                );
            }
            Ctl::Stepper {
                min, max, get, fmt, ..
            } => {
                let v = get(st);
                let bw = 30.0;
                let vw = 92.0;
                let plus = Rect::new(cr.right() - bw, cr.cy() - bw / 2.0, bw, bw);
                let val = Rect::new(plus.x - vw, cr.y, vw, cr.h);
                let minus = Rect::new(val.x - bw, plus.y, bw, bw);
                for (b, icon, sub, enabled) in [
                    (minus, Icon::Minus, 1u16, v > *min + 1e-3),
                    (plus, Icon::Plus, 2u16, v < *max - 1e-3),
                ] {
                    let a = Action::Settings(id, sub);
                    let hv = enabled && cv.hovered(b);
                    let pressed = enabled && self.pressed.as_ref() == Some(&a);
                    cv.push_alpha(if enabled { 1.0 } else { DISABLED_ALPHA });
                    cv.push_tf(0.0, 0.0, if pressed { 0.92 } else { 1.0 }, b.cx(), b.cy());
                    cv.circle(
                        b.cx(),
                        b.cy(),
                        bw / 2.0,
                        TRACK.with_alpha(if hv { 1.5 } else { 1.0 }),
                    );
                    cv.icon(icon, b.cx(), b.cy(), 13.0, cv.theme.ink);
                    cv.pop_tf();
                    cv.pop_alpha();
                    if enabled {
                        cv.zone_named(b, a, true, if sub == 1 { "Less" } else { "More" }, bw / 2.0);
                    }
                }
                cv.text(&fmt(v), &text::SET_VALUE, val, cv.theme.ink, Align::Center);
            }
            Ctl::Text(get, _) => {
                if self.focus != Some(id) && !self.fields.contains_key(&id) {
                    let mut f = TextField::default();
                    f.set(&get(st));
                    self.fields.insert(id, f);
                }
                if self.focus != Some(id) {
                    if let Some(f) = self.fields.get_mut(&id) {
                        let cur = get(st);
                        if f.text != cur {
                            f.set(&cur);
                        }
                    }
                }
                self.field(
                    cv,
                    id,
                    Rect::new(cr.x, cr.cy() - 15.0, cr.w, 30.0),
                    "",
                    None,
                );
            }
            Ctl::List(get, _) => {
                if self.focus != Some(id) {
                    let mut f = TextField::default();
                    f.set(&get(st).join("\n"));
                    self.fields.insert(id, f);
                }
                self.field(
                    cv,
                    id,
                    Rect::new(cr.x, r.y + 10.0, cr.w, r.h - 20.0),
                    "One per line",
                    None,
                );
            }
            Ctl::Secret(name) => {
                let has = crate::platform::credentials::has_secret(name);
                if self.focus == Some(id) {
                    self.field(
                        cv,
                        id,
                        Rect::new(cr.x, cr.cy() - 15.0, cr.w, 30.0),
                        "Paste key, then Enter",
                        None,
                    );
                } else {
                    let r2 = Rect::new(cr.x, cr.cy() - 15.0, cr.w, 30.0);
                    cv.sq(r2, R_INPUT, cv.theme.surface_raised);
                    cv.text(
                        if has {
                            "••••••••••••  (saved)"
                        } else {
                            "Not set — click to add"
                        },
                        &text::BODY,
                        r2.inset_xy(12.0, 0.0),
                        if has { cv.theme.ink } else { FAINT },
                        Align::Left,
                    );
                    cv.zone_named(
                        r2,
                        Action::Focus(InputId::Settings(id)),
                        true,
                        item.label,
                        R_INPUT,
                    );
                }
            }
            Ctl::Hotkey(get, _) => {
                let rec = self.recording == Some(id);
                let label = if rec {
                    crate::i18n::tr("Press keys…")
                } else {
                    get(st)
                };
                let hw = cv.measure(&label, &text::MONO) + 32.0;
                let b = Rect::new(cr.right() - hw, cr.cy() - 14.0, hw, 28.0);
                cv.sq(
                    b,
                    10.0,
                    if rec {
                        cv.theme.accent[1].with_alpha(0.4)
                    } else {
                        TRACK
                    },
                );
                cv.text(&label, &text::MONO, b, cv.theme.ink, Align::Center);
                cv.zone_named(b, Action::Settings(id, 0), true, item.label, 10.0);
            }
            Ctl::Button(label, _) => {
                let bw = cv.pill_width(label, 80.0);
                cv.pill_button(
                    label,
                    Rect::new(cr.right() - bw, cr.cy() - 14.0, bw, 28.0),
                    Action::Settings(id, 0),
                    false,
                );
            }
            Ctl::Confirm(label, _) => {
                let armed = self.confirm.is_some_and(|c| c.0 == id);
                if armed {
                    let cw = cv.pill_width("Cancel", 76.0);
                    let c = Rect::new(cr.right() - cw, cr.cy() - 14.0, cw, 28.0);
                    cv.pill_button("Cancel", c, Action::Settings(id, 2), false);
                    let dw = cv.pill_width(label, 80.0);
                    let d = Rect::new(c.x - 8.0 - dw, c.y, dw, 28.0);
                    danger_button(cv, label, d, Action::Settings(id, 1));
                } else {
                    let bw = cv.pill_width(label, 80.0);
                    cv.pill_button(
                        label,
                        Rect::new(cr.right() - bw, cr.cy() - 14.0, bw, 28.0),
                        Action::Settings(id, 0),
                        false,
                    );
                }
            }
            Ctl::Account(state, _) => {
                let s = state(st);
                let label = match s {
                    AccountState::Connected => "Disconnect",
                    _ => "Connect",
                };
                let bw = cv.pill_width(label, 96.0);
                let br = Rect::new(cr.right() - bw, cr.cy() - 14.0, bw, 28.0);
                cv.pill_button(
                    label,
                    br,
                    Action::Settings(id, 0),
                    s == AccountState::Disconnected,
                );
                let (dot, text_c, status) = match s {
                    AccountState::Connected => (GOOD, GOOD, "Connected"),
                    AccountState::Pending => (WARN, cv.theme.mist, "Waiting for browser…"),
                    AccountState::Disconnected => (FAINT, cv.theme.mist, "Not connected"),
                };
                let tr = crate::i18n::tr(status);
                let tw = cv.measure(&tr, &text::CAPTION.weight(600));
                let tx = br.x - 12.0 - tw;
                if s == AccountState::Connected {
                    cv.rrect(
                        Rect::new(tx - 22.0, cr.cy() - 11.0, tw + 30.0, 22.0),
                        11.0,
                        GOOD.with_alpha(0.14),
                    );
                }
                cv.circle(tx - 10.0, cr.cy(), 3.5, dot);
                cv.text(
                    &tr,
                    &text::CAPTION.weight(600),
                    Rect::new(tx, cr.y, tw + 2.0, cr.h),
                    text_c,
                    Align::Left,
                );
            }
            Ctl::Info(f) => {
                let t = f(st);
                let x = r.x + r.w * 0.36;
                // a warning reads as one
                let c = if t.starts_with('⚠') {
                    WARN
                } else if t.starts_with("AA ✓") {
                    GOOD
                } else {
                    cv.theme.mist
                };
                cv.text_top(
                    &t,
                    &text::SET_DESC.lines(3),
                    x,
                    r.y + 10.0,
                    r.right() - SET_CARD_PAD - x,
                    c,
                    Align::Right,
                );
            }
            Ctl::Chips(options, get, _) => {
                let o = options(st);
                let cur = get(st);
                let row = Rect::new(
                    r.x + SET_CARD_PAD,
                    r.y + 38.0,
                    r.w - 2.0 * SET_CARD_PAD,
                    50.0,
                );
                let n = o.len().max(1) as f32;
                let gap = 8.0;
                let w = (row.w - gap * (n - 1.0)) / n;
                for (i, name) in o.iter().enumerate() {
                    let c = Rect::new(row.x + i as f32 * (w + gap), row.y, w, row.h);
                    let a = Action::Settings(id, i as u16 + 1);
                    let hover = cv.hovered(c);
                    let (hv, pv) = cv.anim(&a, hover, false);
                    cv.push_tf(0.0, 0.0, 1.0 + 0.02 * hv - 0.03 * pv, c.cx(), c.cy());
                    let (s, _, _, rim) = GlassPreset::ALL
                        .get(i)
                        .and_then(|p| p.values())
                        .unwrap_or((0.5, 0.5, 0.5, 0.5));
                    if let Some(g) = cv.gradient(
                        crate::render::canvas::v2(c.x, c.y),
                        crate::render::canvas::v2(c.right(), c.bottom()),
                        &[
                            (0.0, PREVIEW_WALL[0].with_alpha(0.55)),
                            (1.0, PREVIEW_WALL[1].with_alpha(0.55)),
                        ],
                        1.0,
                    ) {
                        cv.rrect_brush(c, 12.0, crate::render::canvas::as_brush(&g));
                    }
                    cv.rrect(c, 12.0, GLASS_TINT.with_alpha(0.15 + 0.8 * s));
                    cv.stroke_rrect(
                        c.inset(0.75),
                        11.25,
                        WHITE.with_alpha(0.12 + 0.45 * rim),
                        1.2,
                    );
                    if i == cur {
                        cv.rrect(c, 12.0, cv.theme.accent[0].with_alpha(0.08));
                        cv.stroke_rrect(c.inset(-2.0), 14.0, cv.theme.accent[0], CHIP_RIM);
                        cv.circle(c.right() - 10.0, c.y + 10.0, 7.0, cv.theme.accent[0]);
                        cv.icon(Icon::Check, c.right() - 10.0, c.y + 10.0, 9.0, WHITE);
                    }
                    cv.text(
                        &crate::i18n::tr(name),
                        &text::CAPTION.weight(600),
                        c,
                        WHITE,
                        Align::Center,
                    );
                    cv.pop_tf();
                    cv.zone_named(c, a, true, name, 12.0);
                }
            }
        }
    }

    /// The segmented control: a raised thumb slides between segments and stretches toward
    /// where it's going.
    pub(super) fn segmented(
        &mut self,
        cv: &mut Canvas,
        id: u16,
        label: &str,
        o: &[String],
        cur: usize,
        cr: Rect,
    ) {
        let names: Vec<String> = o.iter().map(|s| crate::i18n::tr(s)).collect();
        let natural: f32 = names
            .iter()
            .map(|s| cv.measure(s, &text::CAPTION.weight(600)) + 26.0)
            .fold(0.0, f32::max)
            * names.len() as f32;
        let sw = natural.clamp(120.0, cr.w.min(300.0));
        let seg = Rect::new(cr.right() - sw, cr.cy() - 15.0, sw, 30.0);
        cv.rrect(seg, 15.0, TRACK);
        let n = names.len().max(1) as f32;
        let cw = sw / n;
        let th = self
            .thumbs
            .entry(id)
            .or_insert_with(|| Spring::unit(cur as f32, SNAPPY));
        th.set(cur as f32);
        let (v, vel) = (th.value, th.velocity);
        let stretch = (vel.abs() * 0.02).min(SEG_STRETCH) * cw;
        let x = seg.x + v * cw + 2.0 - if vel < 0.0 { stretch } else { 0.0 };
        let thumb = Rect::new(x, seg.y + 2.0, cw - 4.0 + stretch, seg.h - 4.0);
        cv.rrect(thumb.translate(0.0, 0.8), 13.0, NOTCH.with_alpha(0.22));
        cv.rrect(thumb, 13.0, cv.theme.surface_raised.lerp(INK, 0.16));
        cv.stroke_rrect(thumb.inset(0.5), 12.5, WHITE.with_alpha(0.10), 1.0);
        for (i, s) in names.iter().enumerate() {
            let rr = Rect::new(seg.x + i as f32 * cw, seg.y, cw, seg.h);
            let near = 1.0 - (v - i as f32).abs().min(1.0);
            cv.text(
                s,
                &text::CAPTION.weight(600),
                rr,
                cv.theme.mist.lerp(cv.theme.ink, near),
                Align::Center,
            );
            cv.zone_named(
                rr,
                Action::Settings(id, i as u16 + 1),
                true,
                &format!("{label}: {s}"),
                13.0,
            );
        }
    }

    /// The slider: track (thicker on hover), accent fill, a knob with a rim light, a value
    /// bubble while dragging and a little pop when it snaps onto a detent.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn slider_ctl(
        &self,
        cv: &mut Canvas,
        id: u16,
        label: &str,
        r: Rect,
        frac: f32,
        value: &str,
        detents: bool,
    ) {
        let a = Action::Slider(SliderKind::SettingsSlider(id));
        let dragging = self.slider.is_some_and(|s| s.0 == id);
        let hover = dragging || cv.hovered(r.inset_xy(-4.0, -6.0));
        let th = if hover {
            SLIDER_TRACK_HOVER
        } else {
            SLIDER_TRACK
        };
        let track = Rect::new(r.x, r.cy() - th / 2.0, r.w, th);
        cv.rrect(track, th / 2.0, TRACK);
        let x = r.x + r.w * frac;
        if frac > 0.0 {
            if let Some(b) = cv.accent_brush(r.x, r.right(), 1.0) {
                cv.rrect_brush(
                    Rect::new(r.x, track.y, (x - r.x).max(th), th),
                    th / 2.0,
                    crate::render::canvas::as_brush(&b),
                );
            }
        }
        if detents {
            for d in DETENTS {
                cv.circle(
                    r.x + r.w * d,
                    r.cy() + 10.0,
                    1.6,
                    cv.theme.mist.with_alpha(0.7),
                );
            }
        }
        let pop = self
            .snap
            .filter(|s| s.0 == id)
            .map(|s| {
                let k = (s.1.elapsed().as_secs_f32() * 1000.0 / SNAP_MS).min(1.0);
                1.0 + 0.15 * (1.0 - k) * (1.0 - k)
            })
            .unwrap_or(1.0);
        let kr = SLIDER_KNOB / 2.0 * pop * if dragging { 1.06 } else { 1.0 };
        cv.circle(x, r.cy() + 1.0, kr + 0.5, NOTCH.with_alpha(0.3));
        cv.circle(x, r.cy(), kr, WHITE);
        cv.stroke_circle(
            x,
            r.cy(),
            kr - 0.6,
            cv.theme.accent[0].with_alpha(0.35),
            1.2,
        );
        if dragging {
            let tw = cv.measure(value, &text::CAPTION_TNUM) + 16.0;
            let b = Rect::new(x - tw / 2.0, r.cy() - kr - 30.0, tw, 22.0);
            cv.sq(b, 8.0, cv.theme.surface_raised.lerp(INK, 0.1));
            cv.text(value, &text::CAPTION_TNUM, b, cv.theme.ink, Align::Center);
        }
        cv.zone_named(r.inset_xy(-SLIDER_KNOB / 2.0, -8.0), a, true, label, 8.0);
    }

    /// The theme gallery: a card per theme with its surface, a mini notch and its accent.
    pub(super) fn gallery(&self, st: &State, cv: &mut Canvas, id: u16, r: Rect) {
        let themes = super::studio::all_themes(st);
        let installed = themes.len();
        // the gallery's not-yet-installed themes follow, marked with a +
        let extras: Vec<crate::theme::Theme> = super::studio::gallery_extras(st)
            .iter()
            .filter_map(|p| p.to_theme().ok())
            .collect();
        let active = st.theme_id().to_string();
        let (cols, _) = self.gallery_grid();
        let area = Rect::new(
            r.x + SET_CARD_PAD,
            r.y + 12.0,
            r.w - 2.0 * SET_CARD_PAD,
            r.h - 24.0,
        );
        let cw = (area.w - THEME_GAP * (cols as f32 - 1.0)) / cols as f32;
        for (i, t) in themes.iter().chain(extras.iter()).enumerate() {
            let extra = i >= installed;
            let (cx, cy) = (i % cols, i / cols);
            let c = Rect::new(
                area.x + cx as f32 * (cw + THEME_GAP),
                area.y + cy as f32 * (THEME_CARD_H + THEME_GAP),
                cw,
                THEME_CARD_H,
            );
            if !cv.visible(c) {
                continue;
            }
            let a = Action::Settings(id, i as u16 + 1);
            let hover = cv.hovered(c);
            let (hv, pv) = cv.anim(&a, hover, false);
            cv.push_tf(0.0, 0.0, 1.0 + 0.025 * hv - 0.03 * pv, c.cx(), c.cy());
            cv.rrect(c, 14.0, t.surface);
            cv.stroke_rrect(c.inset(0.5), 13.5, WHITE.with_alpha(0.08 + 0.06 * hv), 1.0);
            // a mini notch carrying the accent gradient
            let n = Rect::new(c.cx() - c.w * 0.3, c.y, c.w * 0.6, 22.0);
            cv.push_clip(c);
            cv.rrect(Rect::new(n.x, n.y - 10.0, n.w, n.h + 10.0), 10.0, NOTCH);
            cv.pop_clip();
            if let Some(g) = cv.gradient(
                crate::render::canvas::v2(n.x + 10.0, 0.0),
                crate::render::canvas::v2(n.right() - 10.0, 0.0),
                &[(0.0, t.accent[0]), (0.5, t.accent[1]), (1.0, t.accent[2])],
                1.0,
            ) {
                cv.rrect_brush(
                    Rect::new(n.x + 12.0, n.cy() - 1.5, n.w - 24.0, 3.0),
                    1.5,
                    crate::render::canvas::as_brush(&g),
                );
            }
            // a sample tile in the raised surface with the theme's text
            let tile = Rect::new(c.x + 10.0, c.y + 30.0, c.w - 20.0, 18.0);
            cv.rrect(tile, 6.0, t.surface_raised);
            cv.rrect(
                Rect::new(tile.x + 6.0, tile.cy() - 2.0, tile.w * 0.45, 4.0),
                2.0,
                t.ink.with_alpha(0.8),
            );
            cv.circle(tile.right() - 9.0, tile.cy(), 4.0, t.accent[1]);
            cv.text(
                &t.name,
                &text::CAPTION.weight(600),
                Rect::new(c.x + 10.0, c.bottom() - 26.0, c.w - 20.0, 20.0),
                t.ink,
                Align::Left,
            );
            if extra {
                cv.circle(c.right() - 12.0, c.y + 12.0, 8.0, WHITE.with_alpha(0.16));
                cv.icon(Icon::Plus, c.right() - 12.0, c.y + 12.0, 10.0, WHITE);
            } else if st.studio.is_user(&t.id) {
                cv.circle(c.right() - 12.0, c.bottom() - 16.0, 3.0, t.accent[0]);
            }
            if !extra && t.id == active {
                cv.stroke_rrect(c.inset(-2.5), 16.0, cv.theme.accent[0], 2.0);
                cv.circle(c.right() - 12.0, c.y + 12.0, 8.0, cv.theme.accent[0]);
                cv.icon(Icon::Check, c.right() - 12.0, c.y + 12.0, 10.0, WHITE);
            }
            cv.pop_tf();
            let label = if extra {
                format!("{} ({})", t.name, crate::i18n::tr("add from gallery"))
            } else {
                t.name.clone()
            };
            cv.zone_named(c, a, true, &label, 14.0);
        }
    }

    /// The colour picker: saturation/value square, hue strip, hex field, recent colours.
    pub(super) fn draw_picker(&mut self, st: &State, cv: &mut Canvas) {
        let Some(r) = self.picker_rect() else { return };
        let [h, s, v] = self.hsv;
        cv.sq(r.inset(-1.0), 17.0, NOTCH.with_alpha(0.35));
        cv.sq(r, 16.0, cv.theme.surface_raised);
        cv.stroke_rrect(r.inset(0.5), 15.5, LINE.with_alpha(1.6), 1.0);
        let pad = 14.0;
        let sq = Rect::new(r.x + pad, r.y + pad, r.w - 2.0 * pad, 150.0);
        cv.rrect(sq, 10.0, hsv_to_rgb(h, 1.0, 1.0));
        if let Some(g) = cv.gradient(
            crate::render::canvas::v2(sq.x, 0.0),
            crate::render::canvas::v2(sq.right(), 0.0),
            &[(0.0, WHITE), (1.0, WHITE.with_alpha(0.0))],
            1.0,
        ) {
            cv.rrect_brush(sq, 10.0, crate::render::canvas::as_brush(&g));
        }
        if let Some(g) = cv.gradient(
            crate::render::canvas::v2(0.0, sq.y),
            crate::render::canvas::v2(0.0, sq.bottom()),
            &[(0.0, NOTCH.with_alpha(0.0)), (1.0, NOTCH)],
            1.0,
        ) {
            cv.rrect_brush(sq, 10.0, crate::render::canvas::as_brush(&g));
        }
        let (mx, my) = (sq.x + s * sq.w, sq.y + (1.0 - v) * sq.h);
        cv.stroke_circle(mx, my, 7.0, WHITE, 2.0);
        cv.stroke_circle(mx, my, 8.5, NOTCH.with_alpha(0.4), 1.0);
        cv.zone_named(
            sq,
            Action::Settings(PICK, PICK_SV),
            false,
            "Saturation and brightness",
            10.0,
        );
        // hue strip
        let hs = Rect::new(sq.x, sq.bottom() + 12.0, sq.w, 14.0);
        let stops: Vec<(f32, Color)> = (0..=6)
            .map(|i| (i as f32 / 6.0, hsv_to_rgb(i as f32 * 60.0, 1.0, 1.0)))
            .collect();
        if let Some(g) = cv.gradient(
            crate::render::canvas::v2(hs.x, 0.0),
            crate::render::canvas::v2(hs.right(), 0.0),
            &stops,
            1.0,
        ) {
            cv.rrect_brush(hs, 7.0, crate::render::canvas::as_brush(&g));
        }
        let hx = hs.x + h / 360.0 * hs.w;
        cv.circle(hx, hs.cy(), 8.0, WHITE);
        cv.circle(hx, hs.cy(), 6.0, hsv_to_rgb(h, 1.0, 1.0));
        cv.zone_named(
            hs.inset_xy(0.0, -4.0),
            Action::Settings(PICK, PICK_HUE),
            false,
            "Hue",
            7.0,
        );
        // hex field (shakes when it can't be read) and the current colour
        let shake = self
            .shake
            .map(|t| {
                let e = t.elapsed().as_secs_f32() * 1000.0;
                if e < SHAKE_MS {
                    (e / SHAKE_MS * 3.0 * std::f32::consts::TAU).sin() * 4.0 * (1.0 - e / SHAKE_MS)
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        let fr = Rect::new(sq.x + 40.0 + shake, hs.bottom() + 12.0, 130.0, 30.0);
        let cur = hsv_to_rgb(h, s, v);
        cv.circle(sq.x + 15.0, fr.cy(), 14.0, cur);
        cv.stroke_circle(sq.x + 15.0, fr.cy(), 14.0, WHITE.with_alpha(0.25), 1.0);
        if self.focus != Some(HEX_ID) {
            let mut f = TextField::default();
            f.set(&crate::theme::studio::hex(cur));
            self.fields.insert(HEX_ID, f);
        }
        self.field(cv, HEX_ID, fr, "#RRGGBB", None);
        cv.stroke_rrect(fr.inset(0.5), R_INPUT, LINE.with_alpha(2.0), 1.0);
        if self
            .shake
            .is_some_and(|t| t.elapsed().as_secs_f32() * 1000.0 < SHAKE_MS)
        {
            cv.stroke_rrect(fr.inset(0.5), R_INPUT, CRITICAL, 1.5);
        }
        // recent colours
        let ry = fr.bottom() + 12.0;
        for (i, hx) in st.studio.recent.iter().enumerate() {
            let Some(c) = Color::parse(hx) else { continue };
            let cx = sq.x + 11.0 + i as f32 * 29.0;
            let a = Action::Settings(PICK, PICK_RECENT + i as u16);
            let hv = cv.hovered(Rect::centered(cx, ry + 11.0, 22.0, 22.0));
            cv.circle(cx, ry + 11.0, if hv { 11.0 } else { 10.0 }, c);
            cv.stroke_circle(cx, ry + 11.0, 10.0, WHITE.with_alpha(0.2), 1.0);
            cv.zone_named(Rect::centered(cx, ry + 11.0, 22.0, 22.0), a, true, hx, 11.0);
        }
        if st.studio.recent.is_empty() {
            cv.text(
                &crate::i18n::tr("Recent colours show here"),
                &text::SET_DESC,
                Rect::new(sq.x, ry, sq.w, 22.0),
                FAINT,
                Align::Left,
            );
        }
    }
}
