//! Shared QR scanner icon used by the app window and Windows tray.
use eframe::egui;

const BACKGROUND: [u8; 4] = [20, 126, 128, 255];
const MODULE: [u8; 4] = [248, 252, 250, 255];
const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];

pub(crate) fn icon_data() -> egui::IconData {
    let size = 128;
    egui::IconData {
        rgba: rgba(size),
        width: size,
        height: size,
    }
}

pub(crate) fn rgba(size: u32) -> Vec<u8> {
    let mut pixels = vec![0; (size * size * 4) as usize];
    let unit = size as f32 / 25.0;
    let corner_radius = size as f32 * 0.22;

    for y in 0..size {
        for x in 0..size {
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

            let grid_x = ((px / unit).floor() as i32) - 2;
            let grid_y = ((py / unit).floor() as i32) - 2;
            if (0..21).contains(&grid_x)
                && (0..21).contains(&grid_y)
                && is_qr_module(grid_x, grid_y)
            {
                color = MODULE;
            }

            let offset = ((y * size + x) * 4) as usize;
            pixels[offset..offset + 4].copy_from_slice(&color);
        }
    }

    pixels
}

fn is_qr_module(x: i32, y: i32) -> bool {
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

    // A stable, high-contrast pattern fills the data area without pretending to encode content.
    (x * 7 + y * 11 + x * y * 3) % 13 < 6
}
