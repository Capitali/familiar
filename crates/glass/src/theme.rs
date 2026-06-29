//! The Glass theme — the "future-retro" cockpit.
//!
//! Phase 1 of the redesign (see `docs/UI-DESIGN-BRIEF.md`): the design vocabulary every
//! later phase draws on — the fonts (Spectral, the familiar's serif *voice*; IBM Plex Mono,
//! the *machinery*), the full colour-token palette, and the dimensional helpers (the molded
//! chassis, recessed navy instrument screens, raised cream cards, the blue pill button, the
//! segmented Three-Laws meter, the confidence badge). egui can't do CSS gradients or
//! blend-modes, so the dimensional look is a faithful *interpretation*: layered fills,
//! stepped colours, highlight/shadow strokes, and soft drop shadows.
//!
//! Many helpers here are consumed by phases 2–4 (shell/layout, panel re-skin, polish), so
//! the module allows dead code while the redesign is in flight.
#![allow(dead_code)]

use egui::{Color32, CornerRadius, FontFamily, FontId, Frame, Margin, Shadow, Stroke};

// ---------------------------------------------------------------------------------------
// Colour tokens — straight from the brief's design system.
// ---------------------------------------------------------------------------------------

// Chassis (outer molded shell, light → dark)
pub const CHASSIS_LIGHT: Color32 = Color32::from_rgb(0xe9, 0xdc, 0xc0);
pub const CHASSIS_MID: Color32 = Color32::from_rgb(0xd8, 0xc6, 0x9e);
pub const CHASSIS_DARK: Color32 = Color32::from_rgb(0xca, 0xb9, 0x8f);

// Bezel / panel cream
pub const CREAM: Color32 = Color32::from_rgb(0xe7, 0xda, 0xbe);
pub const CREAM_LIGHT: Color32 = Color32::from_rgb(0xec, 0xe1, 0xc8);
pub const CREAM_CARD: Color32 = Color32::from_rgb(0xe6, 0xd8, 0xb8);
pub const CARD_TOP: Color32 = Color32::from_rgb(0xec, 0xdf, 0xc0); // raised card gradient top
pub const CARD_BOT: Color32 = Color32::from_rgb(0xe2, 0xd4, 0xad); // raised card gradient bottom

// Rails
pub const RAIL_LIGHT: Color32 = Color32::from_rgb(0xdc, 0xcb, 0xa4);
pub const RAIL_DARK: Color32 = Color32::from_rgb(0xcd, 0xbb, 0x91);

// Borders / hairlines
pub const HAIRLINE: Color32 = Color32::from_rgb(0xbc, 0xa8, 0x82);
pub const HAIRLINE_LIGHT: Color32 = Color32::from_rgb(0xcd, 0xbd, 0x99);
pub const HAIRLINE_WARM: Color32 = Color32::from_rgb(0xc8, 0xb8, 0x90);

// Ink (text on cream/beige)
pub const INK: Color32 = Color32::from_rgb(0x33, 0x29, 0x1a);
pub const INK_HEAD: Color32 = Color32::from_rgb(0x34, 0x30, 0x1f);
pub const INK_LABEL: Color32 = Color32::from_rgb(0x7e, 0x70, 0x48);
pub const INK_LABEL2: Color32 = Color32::from_rgb(0x6f, 0x62, 0x44);
pub const INK_MUTED: Color32 = Color32::from_rgb(0x8a, 0x7c, 0x57);
pub const INK_MUTED2: Color32 = Color32::from_rgb(0xa3, 0x94, 0x6d);

// Instrument navy (recessed screens)
pub const NAVY: Color32 = Color32::from_rgb(0x16, 0x1f, 0x33);
pub const NAVY_MID: Color32 = Color32::from_rgb(0x1e, 0x29, 0x40);
pub const NAVY_ALT: Color32 = Color32::from_rgb(0x1b, 0x24, 0x38);

// Text on navy (NEVER dark-on-dark — §11.8)
pub const SCREEN_BRIGHT: Color32 = Color32::from_rgb(0xea, 0xf0, 0xfb);
pub const SCREEN_TEXT: Color32 = Color32::from_rgb(0xdb, 0xe3, 0xf2);
pub const SCREEN_DIM: Color32 = Color32::from_rgb(0xae, 0xbc, 0xd6);
pub const SCREEN_FAINT: Color32 = Color32::from_rgb(0x8e, 0xa0, 0xc4);

// Action blue (primary buttons)
pub const BLUE_LIGHT: Color32 = Color32::from_rgb(0x6f, 0xa0, 0xd4);
pub const BLUE_MID: Color32 = Color32::from_rgb(0x3f, 0x6e, 0xa4);
pub const BLUE_DARK: Color32 = Color32::from_rgb(0x34, 0x5f, 0x8e);
pub const BLUE_BORDER: Color32 = Color32::from_rgb(0x27, 0x4a, 0x72);

// Signals / status
pub const GREEN: Color32 = Color32::from_rgb(0x7b, 0xe0, 0x8a); // healthy / known / lit
pub const GREEN_DEEP: Color32 = Color32::from_rgb(0x5f, 0x8a, 0x52);
pub const METER_OFF: Color32 = Color32::from_rgb(0x2b, 0x3a, 0x5a); // unlit segment on navy
pub const AMBER: Color32 = Color32::from_rgb(0xe0, 0xb2, 0x4a);
pub const AMBER_DEEP: Color32 = Color32::from_rgb(0x9a, 0x73, 0x20);
pub const RED: Color32 = Color32::from_rgb(0xe0, 0x66, 0x4a); // alarm / observing / stopped
pub const RED_DEEP: Color32 = Color32::from_rgb(0xc2, 0x56, 0x3f);
pub const CYAN: Color32 = Color32::from_rgb(0x5b, 0xd6, 0xe6); // vision accent
pub const CYAN_DEEP: Color32 = Color32::from_rgb(0x3f, 0xc3, 0xd8);
pub const CYAN_BRIGHT: Color32 = Color32::from_rgb(0xbf, 0xf4, 0xfb);
pub const FROZEN: Color32 = Color32::from_rgb(0x6f, 0x7a, 0x90); // signals when daemon stopped

// Per-signal identity colours (the vital-signs lines — identity, not health)
pub const SIG_SERVICE: Color32 = GREEN;
pub const SIG_PRESENCE: Color32 = BLUE_LIGHT;
pub const SIG_CAPACITIES: Color32 = AMBER;

// Confidence trio (the no-misinformation promise — unmistakable, never alarming)
pub const CONF_KNOWN_TEXT: Color32 = Color32::from_rgb(0x4d, 0x73, 0x44);
pub const CONF_KNOWN_BG: Color32 = Color32::from_rgb(0xe6, 0xea, 0xd8);
pub const CONF_KNOWN_BD: Color32 = Color32::from_rgb(0x9b, 0xbb, 0x8d);
pub const CONF_PROBABLE_TEXT: Color32 = Color32::from_rgb(0x9a, 0x73, 0x20);
pub const CONF_PROBABLE_BG: Color32 = Color32::from_rgb(0xef, 0xe4, 0xc9);
pub const CONF_PROBABLE_BD: Color32 = Color32::from_rgb(0xd3, 0xbd, 0x84);
pub const CONF_UNKNOWN_TEXT: Color32 = Color32::from_rgb(0x7e, 0x72, 0x59);
pub const CONF_UNKNOWN_BG: Color32 = Color32::from_rgb(0xe4, 0xd9, 0xbf);
pub const CONF_UNKNOWN_BD: Color32 = Color32::from_rgb(0xc2, 0xb3, 0x90);

// ---------------------------------------------------------------------------------------
// Fonts — Spectral (serif voice) + IBM Plex Mono (machinery). The machinery font is the
// default for all chrome; the serif is applied explicitly to the familiar's own words.
// ---------------------------------------------------------------------------------------

fn family(name: &str) -> FontFamily {
    FontFamily::Name(name.into())
}

/// IBM Plex Mono at `size` — the default voice of the machinery (labels, numbers, buttons).
pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}
/// IBM Plex Mono Medium.
pub fn mono_med(size: f32) -> FontId {
    FontId::new(size, family("mono-med"))
}
/// IBM Plex Mono SemiBold.
pub fn mono_semi(size: f32) -> FontId {
    FontId::new(size, family("mono-semi"))
}
/// Spectral — the familiar's serif voice (questions, answers, theory, the person's name).
pub fn serif(size: f32) -> FontId {
    FontId::new(size, family("serif"))
}
/// Spectral Medium.
pub fn serif_med(size: f32) -> FontId {
    FontId::new(size, family("serif-med"))
}
/// Spectral SemiBold — the largest serif (the conversation question).
pub fn serif_semi(size: f32) -> FontId {
    FontId::new(size, family("serif-semi"))
}
/// Spectral Italic — tentative / ambient first-person copy.
pub fn serif_italic(size: f32) -> FontId {
    FontId::new(size, family("serif-italic"))
}

/// Register the cockpit's fonts and make IBM Plex Mono the default proportional/monospace
/// face (the machinery), with Spectral available as named families for the voice.
pub fn install_fonts(ctx: &egui::Context) {
    use std::sync::Arc;
    let mut fonts = egui::FontDefinitions::default();
    let add = |fonts: &mut egui::FontDefinitions, key: &str, bytes: &'static [u8]| {
        fonts
            .font_data
            .insert(key.to_owned(), Arc::new(egui::FontData::from_static(bytes)));
    };
    add(&mut fonts, "mono", include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf"));
    add(&mut fonts, "mono-med", include_bytes!("../assets/fonts/IBMPlexMono-Medium.ttf"));
    add(&mut fonts, "mono-semi", include_bytes!("../assets/fonts/IBMPlexMono-SemiBold.ttf"));
    add(&mut fonts, "serif", include_bytes!("../assets/fonts/Spectral-Regular.ttf"));
    add(&mut fonts, "serif-med", include_bytes!("../assets/fonts/Spectral-Medium.ttf"));
    add(&mut fonts, "serif-semi", include_bytes!("../assets/fonts/Spectral-SemiBold.ttf"));
    add(&mut fonts, "serif-italic", include_bytes!("../assets/fonts/Spectral-Italic.ttf"));

    // Machinery is the default: IBM Plex Mono leads both Proportional and Monospace, keeping
    // egui's emoji/fallback fonts after it so glyphs (◐ ◆ ▸ 👁 …) still render.
    for fam in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts.families.entry(fam).or_default().insert(0, "mono".to_owned());
    }
    // Named families for explicit use.
    let fallback: Vec<String> = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (name, key) in [
        ("mono-med", "mono-med"),
        ("mono-semi", "mono-semi"),
        ("serif", "serif"),
        ("serif-med", "serif-med"),
        ("serif-semi", "serif-semi"),
        ("serif-italic", "serif-italic"),
    ] {
        let mut v = vec![key.to_owned()];
        v.extend(fallback.iter().cloned());
        fonts.families.insert(family(name), v);
    }
    ctx.set_fonts(fonts);
}

// ---------------------------------------------------------------------------------------
// Dimensional helpers — egui interpretations of the molded chassis / recessed screens /
// raised cards. (Wired in phases 2–4.)
// ---------------------------------------------------------------------------------------

/// The outer molded chassis surface.
pub fn chassis() -> Frame {
    Frame::new()
        .fill(CHASSIS_MID)
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(14))
        .stroke(Stroke::new(1.0, HAIRLINE))
        .shadow(Shadow {
            offset: [0, 22],
            blur: 48,
            spread: 0,
            color: Color32::from_black_alpha(140),
        })
}

/// A recessed navy instrument screen (meters, charts, toggle cards, the eye).
pub fn instrument_screen() -> Frame {
    Frame::new()
        .fill(NAVY_MID)
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(12))
        .stroke(Stroke::new(1.0, HAIRLINE))
        .shadow(Shadow {
            offset: [0, 2],
            blur: 8,
            spread: 0,
            color: Color32::from_black_alpha(90),
        })
}

/// A raised cream card (transcript items, provider rows, camera rows).
pub fn cream_card() -> Frame {
    Frame::new()
        .fill(CARD_TOP)
        .corner_radius(CornerRadius::same(9))
        .inner_margin(Margin::same(13))
        .stroke(Stroke::new(1.0, HAIRLINE_WARM))
        .shadow(Shadow {
            offset: [0, 2],
            blur: 6,
            spread: 0,
            color: Color32::from_black_alpha(40),
        })
}

/// A flat cream panel (rails, plain surfaces) — no shadow.
pub fn panel(fill: Color32) -> Frame {
    Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(12))
        .stroke(Stroke::new(1.0, HAIRLINE_LIGHT))
}

/// A vertical segmented meter on navy: `value` in 0..1 lights segments bottom-up in
/// `lit`. Draws into the given rect (a recessed well). Used by the Three-Laws meters.
pub fn segmented_meter(ui: &egui::Ui, rect: egui::Rect, value: f64, lit: Color32) {
    const N: usize = 9;
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(4), NAVY);
    let gap = 3.0;
    let seg_h = (rect.height() - gap * (N as f32 + 1.0)) / N as f32;
    let on = ((value.clamp(0.0, 1.0)) * N as f64).round() as usize;
    for i in 0..N {
        // segment 0 is the bottom
        let top = rect.bottom() - gap - (i as f32 + 1.0) * (seg_h + gap) + gap;
        let seg = egui::Rect::from_min_max(
            egui::pos2(rect.left() + gap, top),
            egui::pos2(rect.right() - gap, top + seg_h),
        );
        let color = if i < on { lit } else { METER_OFF };
        p.rect_filled(seg, CornerRadius::same(2), color);
    }
}
