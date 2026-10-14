//! 项目内 SVG 是窗口、托盘和 Windows exe 图标的唯一图标源。
use eframe::egui;

const WINDOW_PNG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app_icon_128.png"));
const TRAY_PNG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app_icon_32.png"));

pub(crate) fn icon_data() -> egui::IconData {
    let image = image::load_from_memory(WINDOW_PNG)
        .expect("build script must rasterize the application SVG")
        .to_rgba8();
    egui::IconData {
        rgba: image.into_raw(),
        width: 128,
        height: 128,
    }
}

pub(crate) fn tray_rgba() -> Vec<u8> {
    image::load_from_memory(TRAY_PNG)
        .expect("build script must rasterize the application SVG")
        .to_rgba8()
        .into_raw()
}
