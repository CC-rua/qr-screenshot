//! 桌面界面共用的颜色与 egui 样式。
use eframe::egui;

pub(crate) const INK: egui::Color32 = egui::Color32::from_rgb(31, 45, 58);
pub(crate) const MUTED: egui::Color32 = egui::Color32::from_rgb(102, 119, 133);
pub(crate) const ACCENT: egui::Color32 = egui::Color32::from_rgb(20, 126, 128);
pub(crate) const ACCENT_HOVER: egui::Color32 = egui::Color32::from_rgb(16, 105, 108);
pub(crate) const CANVAS: egui::Color32 = egui::Color32::from_rgb(244, 247, 249);
pub(crate) const SURFACE: egui::Color32 = egui::Color32::WHITE;
pub(crate) const BORDER: egui::Color32 = egui::Color32::from_rgb(222, 230, 235);

/// Apply the app-wide light palette and comfortable spacing once at startup.
pub(crate) fn apply(ctx: &egui::Context) {
    // 复制当前浅色样式，保留间距等未打算修改的默认设置。
    let mut style = (*ctx.style_of(egui::Theme::Light)).clone();
    // `visuals` 控制颜色和控件状态；下面的 `spacing` 控制布局间距。
    style.visuals = egui::Visuals::light();
    style.visuals.panel_fill = CANVAS;
    style.visuals.window_fill = SURFACE;
    style.visuals.extreme_bg_color = SURFACE;
    style.visuals.faint_bg_color = CANVAS;
    style.visuals.selection.bg_fill = ACCENT;
    style.visuals.selection.stroke = egui::Stroke::new(1.0, SURFACE);
    style.visuals.widgets.noninteractive.bg_fill = CANVAS;
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, INK);
    style.visuals.widgets.inactive.bg_fill = SURFACE;
    style.visuals.widgets.inactive.weak_bg_fill = SURFACE;
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, INK);
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(235, 245, 245);
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, ACCENT_HOVER);
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, ACCENT);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(220, 239, 239);
    style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, ACCENT_HOVER);
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, ACCENT);
    style.visuals.widgets.open.bg_fill = SURFACE;
    style.visuals.override_text_color = Some(INK);
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(13.0, 8.0);
    style.spacing.interact_size = egui::vec2(40.0, 36.0);
    style.visuals.window_corner_radius = egui::CornerRadius::same(12);
    ctx.set_style_of(egui::Theme::Light, style);
}

pub(crate) fn card() -> egui::Frame {
    // 返回可复用的卡片外框，让二维码结果共用相同底色、边框和圆角。
    egui::Frame::group(&egui::Style::default())
        .fill(SURFACE)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin::same(16))
}
