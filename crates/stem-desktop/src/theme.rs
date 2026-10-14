use eframe::egui::{self, Color32};
pub const BACKGROUND: Color32 = Color32::from_rgb(20, 22, 25);
pub const SURFACE: Color32 = Color32::from_rgb(29, 32, 36);
pub const TEXT: Color32 = Color32::from_rgb(235, 235, 231);
pub const MUTED: Color32 = Color32::from_rgb(170, 174, 178);
pub const AMBER: Color32 = Color32::from_rgb(239, 181, 91);
pub const BORDER: Color32 = Color32::from_rgb(118, 123, 131);
pub fn configure(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BACKGROUND;
    visuals.window_fill = SURFACE;
    visuals.override_text_color = Some(TEXT);
    visuals.extreme_bg_color = BACKGROUND;
    visuals.faint_bg_color = SURFACE;
    visuals.selection.bg_fill = Color32::from_rgb(81, 61, 34);
    visuals.selection.stroke = egui::Stroke::new(2.0_f32, AMBER);
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
    ] {
        widget.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
        widget.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);
        widget.corner_radius = egui::CornerRadius::same(4);
    }
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(39, 43, 48);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(52, 57, 64);
    visuals.widgets.active.bg_fill = Color32::from_rgb(73, 55, 31);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(2.0_f32, AMBER);
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10., 8.);
        style.spacing.button_padding = egui::vec2(12., 8.);
        style.spacing.interact_size = egui::vec2(32., 32.);
        for (kind, size) in [
            (egui::TextStyle::Body, 14.),
            (egui::TextStyle::Button, 14.),
            (egui::TextStyle::Small, 12.),
            (egui::TextStyle::Heading, 24.),
        ] {
            style
                .text_styles
                .insert(kind, egui::FontId::proportional(size));
        }
    });
}
