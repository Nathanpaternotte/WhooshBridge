use crate::i18n::t;
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
pub const BLUE: Color32 = Color32::from_rgb(0, 106, 245);
pub const INK: Color32 = Color32::from_rgb(30, 34, 43);
pub const SECONDARY: Color32 = Color32::from_rgb(112, 117, 130);
pub const GREEN: Color32 = Color32::from_rgb(30, 139, 93);
pub const RED: Color32 = Color32::from_rgb(196, 52, 66);
pub const BACKGROUND: Color32 = Color32::from_rgb(243, 245, 250);

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Ok(bytes) = std::fs::read("/System/Library/Fonts/SFNS.ttf") {
        fonts
            .font_data
            .insert("system".into(), egui::FontData::from_owned(bytes).into());
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "system".into());
    }
    ctx.set_fonts(fonts);
    let mut v = egui::Visuals::light();
    v.panel_fill = BACKGROUND;
    v.window_fill = Color32::WHITE;
    v.override_text_color = Some(INK);
    v.weak_text_color = Some(SECONDARY);
    v.extreme_bg_color = Color32::from_rgb(241, 243, 248);
    v.faint_bg_color = Color32::from_rgb(247, 249, 252);
    v.selection.bg_fill = Color32::from_rgb(224, 237, 255);
    v.selection.stroke = Stroke::new(1.0_f32, BLUE);
    v.hyperlink_color = BLUE;
    v.window_corner_radius = 22.into();
    v.menu_corner_radius = 16.into();
    v.window_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = 10.into();
        w.bg_stroke = Stroke::NONE;
        w.fg_stroke = Stroke::new(1.0_f32, INK);
    }
    v.widgets.inactive.bg_fill = Color32::from_rgb(233, 237, 244);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(233, 237, 244);
    v.widgets.hovered.bg_fill = Color32::from_rgb(220, 230, 245);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(224, 233, 247);
    v.widgets.active.bg_fill = Color32::from_rgb(205, 224, 251);
    ctx.set_visuals(v);
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(10.0, 10.0);
    style.spacing.button_padding = Vec2::new(14.0, 9.0);
    style.spacing.interact_size.y = 34.0;
    style.spacing.text_edit_width = 240.0;
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(23.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
    ctx.set_style(style);
}
pub fn surface() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_white_alpha(238))
        .corner_radius(22)
        .stroke(Stroke::new(1.0_f32, Color32::WHITE))
        .inner_margin(22)
        .shadow(egui::Shadow {
            offset: [0, 4],
            blur: 18,
            spread: 0,
            color: Color32::from_black_alpha(8),
        })
}
pub fn pill(ui: &mut egui::Ui, text: &str, connected: bool) {
    egui::Frame::new()
        .fill(if connected {
            Color32::from_rgb(231, 245, 237)
        } else {
            Color32::from_rgb(236, 239, 245)
        })
        .corner_radius(30)
        .inner_margin(egui::Margin::symmetric(11, 6))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("●  {}", t(text)))
                    .size(12.0)
                    .color(if connected { GREEN } else { SECONDARY }),
            );
        });
}
pub fn primary(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(RichText::new(t(text.into())).color(Color32::WHITE).strong())
        .fill(BLUE)
        .corner_radius(30)
        .min_size(Vec2::new(0.0, 42.0))
}

pub fn icon_data() -> egui::IconData {
    let image = image::load_from_memory(include_bytes!("../assets/icon.png"))
        .expect("bundled icon")
        .into_rgba8();
    egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    }
}
