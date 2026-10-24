use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
    Theme, vec2,
};

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

#[derive(Clone, Copy)]
pub struct Pal {
    pub content: Color32,
    pub status: Color32,
    pub panel: Color32,
    pub field: Color32,
    pub hover: Color32,
    pub divider: Color32,
    pub nav_sel: Color32,
    pub line: Color32,
    pub popup_stroke: Color32,
    pub sel: Color32,
    pub pressed: Color32,
    pub control_stroke: Color32,
    pub button: Color32,
    pub button_stroke: Color32,
    pub text: Color32,
    pub weak: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
    pub on_accent: Color32,
    pub error_text: Color32,
    pub eslesme: Color32,
}

pub const KOYU: Pal = Pal {
    content: hex(0x191919),
    status: hex(0x1C1C1C),
    panel: hex(0x202020),
    field: hex(0x2C2C2C),
    hover: hex(0x2D2D2D),
    divider: hex(0x303030),
    nav_sel: hex(0x333333),
    line: hex(0x3A3A3A),
    popup_stroke: hex(0x3C3C3C),
    sel: hex(0x414141),
    pressed: hex(0x424242),
    control_stroke: hex(0x8A8A8A),
    button: hex(0x2D2D2D),
    button_stroke: hex(0x3A3A3A),
    text: hex(0xF0F0F0),
    weak: hex(0xC8C8C8),
    accent: hex(0x0078D4),
    accent_text: hex(0x60CDFF),
    on_accent: hex(0x000000),
    error_text: hex(0xFF99A4),
    eslesme: hex(0x0B3A5C),
};

pub const ACIK: Pal = Pal {
    content: hex(0xFFFFFF),
    status: hex(0xFFFFFF),
    panel: hex(0xF3F3F3),
    field: hex(0xFFFFFF),
    hover: hex(0xF0F0F0),
    divider: hex(0xE5E5E5),
    nav_sel: hex(0xEAEAEA),
    line: hex(0xE5E5E5),
    popup_stroke: hex(0xD7D7D7),
    sel: hex(0xCCE8FF),
    pressed: hex(0xDEDEDE),
    control_stroke: hex(0x8A8A8A),
    button: hex(0xFBFBFB),
    button_stroke: hex(0xDEDEDE),
    text: hex(0x1A1A1A),
    weak: hex(0x606060),
    accent: hex(0x0078D4),
    accent_text: hex(0x005FB8),
    on_accent: hex(0xFFFFFF),
    error_text: hex(0xC42B1C),
    eslesme: hex(0xCCE8FF),
};

pub fn pal(ctx: &egui::Context) -> &'static Pal {
    match ctx.theme() {
        Theme::Dark => &KOYU,
        Theme::Light => &ACIK,
    }
}

pub fn kur(ctx: &egui::Context) {
    yazi_tipleri(ctx);
    ctx.style_mut_of(Theme::Dark, |s| stil(s, &KOYU));
    ctx.style_mut_of(Theme::Light, |s| stil(s, &ACIK));
}

// Segoe UI varsa onu kullan, yoksa egui'nin gömülü yazı tipleri kalır.
fn yazi_tipleri(ctx: &egui::Context) {
    let mut tanim = FontDefinitions::default();
    let klasor = std::env::var_os("WINDIR").map(|w| std::path::PathBuf::from(w).join("Fonts"));
    let mut ekle = |ad: &str, dosyalar: &[&str], aile: FontFamily| {
        let Some(k) = &klasor else { return };
        let Some(veri) = dosyalar.iter().find_map(|d| std::fs::read(k.join(d)).ok()) else {
            return;
        };
        tanim
            .font_data
            .insert(ad.to_owned(), Arc::new(FontData::from_owned(veri)));
        tanim
            .families
            .entry(aile)
            .or_default()
            .insert(0, ad.to_owned());
    };
    ekle("segoe", &["segoeui.ttf"], FontFamily::Proportional);
    ekle(
        "mono",
        &["CascadiaMono.ttf", "consola.ttf"],
        FontFamily::Monospace,
    );
    ctx.set_fonts(tanim);
}

fn stil(s: &mut egui::Style, p: &Pal) {
    s.text_styles = [
        (TextStyle::Small, FontId::proportional(12.0)),
        (TextStyle::Body, FontId::proportional(14.0)),
        (TextStyle::Button, FontId::proportional(14.0)),
        (TextStyle::Heading, FontId::proportional(20.0)),
        (TextStyle::Monospace, FontId::monospace(13.0)),
    ]
    .into();
    s.spacing.item_spacing = vec2(6.0, 4.0);
    s.spacing.button_padding = vec2(8.0, 4.0);
    s.spacing.interact_size.y = 26.0;
    s.spacing.menu_margin = egui::Margin::same(6);
    s.animation_time = 0.0;
    s.scroll_animation = egui::style::ScrollAnimation::none();

    let v = &mut s.visuals;
    v.panel_fill = p.panel;
    v.window_fill = p.field;
    v.extreme_bg_color = p.field;
    v.faint_bg_color = p.hover;
    v.window_stroke = Stroke::new(1.0, p.popup_stroke);
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_shadow = egui::Shadow {
        offset: [0, 8],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(71),
    };
    v.popup_shadow = v.window_shadow;
    v.hyperlink_color = p.accent_text;
    v.error_fg_color = p.error_text;
    v.override_text_color = None;
    v.selection.bg_fill = p.sel;
    v.selection.stroke = Stroke::new(1.0, p.text);
    v.text_cursor.stroke = Stroke::new(2.0, p.accent_text);

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.panel;
    w.noninteractive.weak_bg_fill = p.panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.divider);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.bg_fill = p.button;
    w.inactive.weak_bg_fill = p.button;
    w.inactive.bg_stroke = Stroke::new(1.0, p.button_stroke);
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.hovered.bg_fill = p.hover;
    w.hovered.weak_bg_fill = p.hover;
    w.hovered.bg_stroke = Stroke::NONE;
    w.hovered.fg_stroke = Stroke::new(1.0, p.text);
    w.hovered.expansion = 0.0;
    w.active.bg_fill = p.pressed;
    w.active.weak_bg_fill = p.pressed;
    w.active.bg_stroke = Stroke::NONE;
    w.active.fg_stroke = Stroke::new(1.0, p.text);
    w.active.expansion = 0.0;
    w.open = w.active;
    for durum in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        durum.corner_radius = CornerRadius::same(5);
    }
}
