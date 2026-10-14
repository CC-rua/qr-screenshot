use eframe::egui;
use image::{RgbaImage, imageops};

/// 返回 Result，把无效选区作为可恢复错误交给控制器，而不是抛异常。
/// 教学笔记：[Result 与错误处理](../../learning-notes/rust-java-differences.md#result-errors)。
/// 将选区逻辑坐标映射到截图像素，裁剪后检测并解码其中的二维码。
pub fn crop_and_decode(
    screenshot: &RgbaImage,
    screen_rect: egui::Rect,
    selection: egui::Rect,
) -> Result<Vec<String>, String> {
    let selection = selection.intersect(screen_rect);
    if selection.width() < 8.0 || selection.height() < 8.0 {
        return Err("选区太小，请重新框选。".to_owned());
    }

    let scale_x = screenshot.width() as f32 / screen_rect.width();
    let scale_y = screenshot.height() as f32 / screen_rect.height();
    let x0 = ((selection.left() - screen_rect.left()) * scale_x)
        .floor()
        .max(0.0) as u32;
    let y0 = ((selection.top() - screen_rect.top()) * scale_y)
        .floor()
        .max(0.0) as u32;
    let x1 = ((selection.right() - screen_rect.left()) * scale_x)
        .ceil()
        .min(screenshot.width() as f32) as u32;
    let y1 = ((selection.bottom() - screen_rect.top()) * scale_y)
        .ceil()
        .min(screenshot.height() as f32) as u32;
    if x1 <= x0 || y1 <= y0 {
        return Err("选区无效，请重新框选。".to_owned());
    }

    let crop = imageops::crop_imm(screenshot, x0, y0, x1 - x0, y1 - y0).to_image();
    let grayscale = imageops::grayscale(&crop);
    let mut prepared = rqrr::PreparedImage::prepare(grayscale);
    // 迭代器链逐个解码；filter_map 跳过 Err，collect 收集剩余内容。
    // 教学笔记：[迭代器与 collect](../../learning-notes/rust-java-differences.md#iterators)。
    let contents = prepared
        .detect_grids()
        .into_iter()
        .filter_map(|grid| grid.decode().ok().map(|(_, content)| content))
        .collect();
    Ok(contents)
}
