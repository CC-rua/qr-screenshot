use image::{GrayImage, Luma};
use qrcode::QrCode;
use std::path::Path;

/// 把 UTF-8 文本编码为带标准白色静区的二维码灰度图。
///
/// `QrCode::new` 返回 `Result`：输入过长或编码失败时把错误交给控制器显示。
/// 教学笔记：[Result 与错误处理](../../learning-notes/rust-java-differences.md#result-errors)。
pub fn generate(text: &str) -> Result<GrayImage, String> {
    if text.is_empty() {
        return Err("请输入要生成二维码的内容。".to_owned());
    }

    let code = QrCode::new(text.as_bytes())
        .map_err(|error| format!("无法生成二维码，请缩短内容后重试：{error}"))?;

    // 渲染器会按整数模块尺寸放大，避免二维码小方块被拉伸变形。
    Ok(code.render::<Luma<u8>>().min_dimensions(320, 320).build())
}

/// 将灰度图编码为 PNG 文件，错误路径通过 Result 返回给界面显示。
pub fn save_png(image: &GrayImage, path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("请先填写 PNG 保存路径。".to_owned());
    }
    image
        .save_with_format(path, image::ImageFormat::Png)
        .map_err(|error| format!("保存 PNG 失败：{error}"))
}
