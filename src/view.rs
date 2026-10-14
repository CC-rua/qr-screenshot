use std::time::{Duration, Instant};

use eframe::egui;

use crate::model::{AppModel, MainTab, OverlayState};
use crate::theme;

/// 主窗口把用户操作交给控制器处理，不在视图中直接调用截图或二维码服务。
///
/// 带 `usize` 的 `CopyResult` 将“复制哪一个结果”作为动作数据传给控制器。
/// 教学笔记：[枚举与 match](../learning-notes/rust-java-differences.md#enum-match)。
pub(crate) enum MainAction {
    BeginCapture,
    GenerateQr,
    SaveQr,
    CopyResult(usize),
}

/// 选区遮罩只报告完成或取消；窗口状态恢复由控制器负责。
pub(crate) enum OverlayAction {
    Completed,
    Cancelled,
}

/// 绘制主窗口，并在用户点击按钮时返回一个动作。
///
/// `&mut AppModel` 是可变借用：视图可以编辑输入框和当前选项，但不取得模型所有权。
/// 教学笔记：[借用与可变性](../learning-notes/rust-java-differences.md#ownership-borrowing)。
pub(crate) fn draw_main(model: &mut AppModel, ui: &mut egui::Ui) -> Option<MainAction> {
    // A bottom panel reserves its own fixed height before the active page uses the remaining area.
    // This keeps navigation anchored when switching between pages with different content heights.
    egui::Panel::bottom("main_tab_bar")
        .exact_size(52.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let tab_width = (ui.available_width() - ui.spacing().item_spacing.x) * 0.5;
                if ui
                    .add_sized(
                        [tab_width, 42.0],
                        egui::Button::selectable(model.active_tab == MainTab::Scan, "截图识别"),
                    )
                    .clicked()
                {
                    model.active_tab = MainTab::Scan;
                }
                if ui
                    .add_sized(
                        [ui.available_width(), 42.0],
                        egui::Button::selectable(
                            model.active_tab == MainTab::Generate,
                            "生成二维码",
                        ),
                    )
                    .clicked()
                {
                    model.active_tab = MainTab::Generate;
                }
            });
        });

    let action = match model.active_tab {
        MainTab::Scan => draw_scan_tab(model, ui),
        MainTab::Generate => draw_generate_tab(model, ui),
    };

    if let Some(MainAction::CopyResult(index)) = action.as_ref() {
        model.copied_result = Some(*index);
        model.copy_feedback_until = Some(Instant::now() + Duration::from_secs(2));
    }
    if let Some(until) = model.copy_feedback_until {
        if Instant::now() >= until {
            model.copied_result = None;
            model.copy_feedback_until = None;
        } else {
            ui.ctx().request_repaint_after(until - Instant::now());
        }
    }

    action
}

fn draw_scan_tab(model: &mut AppModel, ui: &mut egui::Ui) -> Option<MainAction> {
    ui.add_space(12.0);
    let action = ui
        .add_sized(
            [ui.available_width(), 48.0],
            egui::Button::new("截图识别")
                .fill(theme::ACCENT)
                .stroke(egui::Stroke::NONE),
        )
        .clicked()
        .then_some(MainAction::BeginCapture);
    ui.add_space(10.0);

    let mut copy_action = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if model.results.is_empty() {
                ui.label(egui::RichText::new("暂无结果").color(theme::MUTED));
                return;
            }
            for (index, content) in model.results.iter().enumerate() {
                theme::card().show(ui, |ui| {
                    // Wrapped horizontal layout keeps the copy button after the text, including long results.
                    ui.horizontal_wrapped(|ui| {
                        ui.add(egui::Label::new(content).wrap());
                        if ui.small_button("复制").clicked() {
                            copy_action = Some(MainAction::CopyResult(index));
                        }
                        if model.copied_result == Some(index)
                            && model
                                .copy_feedback_until
                                .is_some_and(|until| Instant::now() < until)
                        {
                            ui.label("已复制");
                        }
                    });
                });
                ui.add_space(6.0);
            }
        });

    copy_action.or(action)
}

fn draw_generate_tab(model: &mut AppModel, ui: &mut egui::Ui) -> Option<MainAction> {
    ui.add_space(12.0);
    ui.add_sized(
        [ui.available_width(), 180.0],
        egui::TextEdit::multiline(&mut model.qr_text),
    );
    ui.add_space(10.0);
    let mut action = ui
        .add_sized(
            [ui.available_width(), 48.0],
            egui::Button::new("生成二维码")
                .fill(theme::ACCENT)
                .stroke(egui::Stroke::NONE),
        )
        .clicked()
        .then_some(MainAction::GenerateQr);

    if model.generated_qr.is_some() {
        ui.add_space(8.0);
        if ui
            .add_sized(
                [ui.available_width(), 42.0],
                egui::Button::new("另存为 PNG"),
            )
            .clicked()
        {
            action = Some(MainAction::SaveQr);
        }
    }

    if let Some(texture) = model.generated_qr_texture.as_ref() {
        ui.add_space(12.0);
        let max_size = ui
            .available_width()
            .min(ui.available_height())
            .min(240.0)
            .max(1.0);
        let source_size = texture.size_vec2();
        let scale = (max_size / source_size.x.max(source_size.y)).min(1.0);
        ui.vertical_centered(|ui| {
            ui.add(
                egui::Image::new((texture.id(), source_size))
                    .fit_to_exact_size(source_size * scale),
            );
        });
    }

    action
}

/// 绘制全屏选区：截图做背景，暗色遮罩覆盖其上，再突出用户拖出的矩形。
///
/// 该函数只更新选区状态并返回动作；截图和解码仍归控制器及 services 模块处理。
pub(crate) fn draw_selection_overlay(
    ui: &mut egui::Ui,
    overlay: &mut OverlayState,
) -> Option<OverlayAction> {
    if !overlay.active {
        return None;
    }

    // Option 表示纹理可能尚未创建。第一次绘制时由截图像素创建 egui 可显示的纹理。
    if overlay.texture.is_none() {
        if let Some(screenshot) = overlay.screenshot.as_ref() {
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [screenshot.width() as usize, screenshot.height() as usize],
                screenshot.as_raw(),
            );
            overlay.texture = Some(ui.ctx().load_texture(
                "selection-screen-capture",
                color_image,
                egui::TextureOptions::LINEAR,
            ));
        }
    }

    let (Some(screenshot), Some(texture)) = (overlay.screenshot.as_ref(), overlay.texture.as_ref())
    else {
        return None;
    };

    let screen_rect = ui.max_rect();
    let painter = ui.painter();
    let uv = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
    painter.image(texture.id(), screen_rect, uv, egui::Color32::WHITE);
    painter.rect_filled(
        screen_rect,
        egui::CornerRadius::ZERO,
        egui::Color32::from_black_alpha(115),
    );

    if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
        overlay.selection_start = None;
        overlay.selection_current = None;
        overlay.cancelled = true;
        return Some(OverlayAction::Cancelled);
    }

    let pointer_pos = ui.input(|input| input.pointer.interact_pos());
    let primary_pressed = ui.input(|input| input.pointer.primary_pressed());
    let primary_down = ui.input(|input| input.pointer.primary_down());
    let primary_released = ui.input(|input| input.pointer.primary_released());

    if primary_pressed && let Some(position) = pointer_pos {
        if screen_rect.contains(position) {
            overlay.selection_start = Some(position);
            overlay.selection_current = Some(position);
        }
    }
    if primary_down
        && overlay.selection_start.is_some()
        && let Some(position) = pointer_pos
        && screen_rect.contains(position)
    {
        overlay.selection_current = Some(position);
    }

    if let Some(selection) = current_selection(overlay) {
        // 再绘制选区内未变暗的截图，让用户清楚看到框选边界。
        painter.image(
            texture.id(),
            selection,
            uv_for_rect(selection, screen_rect),
            egui::Color32::WHITE,
        );
        painter.rect_stroke(
            selection,
            egui::CornerRadius::ZERO,
            egui::Stroke::new(2.0, egui::Color32::from_rgb(90, 220, 160)),
            egui::StrokeKind::Inside,
        );
    } else {
        painter.text(
            screen_rect.center(),
            egui::Align2::CENTER_CENTER,
            "按住鼠标左键拖动，框选二维码区域\n按 Esc 取消",
            egui::FontId::proportional(20.0),
            egui::Color32::WHITE,
        );
    }

    if primary_released {
        if let (Some(start), Some(end)) = (overlay.selection_start, overlay.selection_current) {
            let selection = egui::Rect::from_two_pos(start, end).intersect(screen_rect);
            // Arc::clone 共享截图像素，不复制整张图片；完成消息由控制器随后取走。
            overlay.completed = Some((std::sync::Arc::clone(screenshot), screen_rect, selection));
            overlay.selection_start = None;
            overlay.selection_current = None;
            return Some(OverlayAction::Completed);
        }
    }

    // 遮罩需要持续刷新，才能跟随拖动指针重绘矩形。
    ui.ctx().request_repaint();
    None
}

fn current_selection(overlay: &OverlayState) -> Option<egui::Rect> {
    Some(egui::Rect::from_two_pos(
        overlay.selection_start?,
        overlay.selection_current?,
    ))
}

/// 将逻辑坐标矩形换算成截图纹理的 0..1 UV 范围。
fn uv_for_rect(rect: egui::Rect, screen_rect: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(
            (rect.left() - screen_rect.left()) / screen_rect.width(),
            (rect.top() - screen_rect.top()) / screen_rect.height(),
        ),
        egui::pos2(
            (rect.right() - screen_rect.left()) / screen_rect.width(),
            (rect.bottom() - screen_rect.top()) / screen_rect.height(),
        ),
    )
}
