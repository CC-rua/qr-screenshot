//! 主窗口与 Windows 托盘共用的二维码图标。
use eframe::egui;

const BACKGROUND: [u8; 4] = [20, 126, 128, 255];
const MODULE: [u8; 4] = [248, 252, 250, 255];
const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];

pub(crate) fn icon_data() -> egui::IconData {
    let size = 128;
    // egui 的图标数据按行保存 RGBA 字节，并单独记录图像的像素宽高。
    egui::IconData {
        rgba: rgba(size),
        width: size,
        height: size,
    }
}

pub(crate) fn rgba(size: u32) -> Vec<u8> {
    let mut pixels = vec![0; (size * size * 4) as usize];
    // 图案使用 21×21 个二维码单元，四周各留两格边距。
    // `unit` 把逻辑单元缩放到不同尺寸，因此窗口和 32px 托盘图标能复用同一份图案。
    let unit = size as f32 / 25.0;
    let corner_radius = size as f32 * 0.22;

    for y in 0..size {
        for x in 0..size {
            // 圆角方块以外保持透明，让 Windows 按圆角形状显示图标。
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let dx = if px < corner_radius {
                corner_radius - px
            } else if px > size as f32 - corner_radius {
                px - (size as f32 - corner_radius)
            } else {
                0.0
            };
            let dy = if py < corner_radius {
                corner_radius - py
            } else if py > size as f32 - corner_radius {
                py - (size as f32 - corner_radius)
            } else {
                0.0
            };
            let mut color = if dx * dx + dy * dy <= corner_radius * corner_radius {
                BACKGROUND
            } else {
                TRANSPARENT
            };

            // 把像素坐标换算成二维码单元坐标；减去 2 是因为图案前面留了两格边距。
            let grid_x = ((px / unit).floor() as i32) - 2;
            let grid_y = ((py / unit).floor() as i32) - 2;
            if (0..21).contains(&grid_x)
                && (0..21).contains(&grid_y)
                && is_qr_module(grid_x, grid_y)
            {
                color = MODULE;
            }

            // 一个像素连续占四个字节，顺序为红、绿、蓝、透明度。
            let offset = ((y * size + x) * 4) as usize;
            pixels[offset..offset + 4].copy_from_slice(&color);
        }
    }

    pixels
}

fn is_qr_module(x: i32, y: i32) -> bool {
    // 三个 7×7 定位图案对应二维码常见的三个角。
    for (left, top) in [(0, 0), (14, 0), (0, 14)] {
        let dx = x - left;
        let dy = y - top;
        if (0..7).contains(&dx) && (0..7).contains(&dy) {
            return dx == 0
                || dx == 6
                || dy == 0
                || dy == 6
                || (2..=4).contains(&dx) && (2..=4).contains(&dy);
        }
    }

    // 用固定图案填充数据区；这是装饰图标，不代表它编码了真实内容。
    (x * 7 + y * 11 + x * y * 3) % 13 < 6
}
