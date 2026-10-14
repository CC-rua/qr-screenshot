#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// 教学笔记：[模块声明与 use](../learning-notes/rust-java-differences.md#modules-use)。
mod app_icon;
mod controller;
mod model;
mod services;
#[cfg(windows)]
mod system_tray;
mod theme;
mod view;

use controller::QrController;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{path::PathBuf, sync::Arc};

/// 程序入口：配置字体、读取原生窗口句柄并启动 MVC 控制器。
fn main() -> eframe::Result {
    eframe::run_native(
        "桌面二维码识别",
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_inner_size([460.0, 600.0])
                .with_resizable(false)
                .with_icon(Arc::new(app_icon::icon_data())),
            ..Default::default()
        },
        Box::new(|creation_context| {
            let system_font_found = install_system_chinese_font(&creation_context.egui_ctx);
            // `window_handle` 来自 HasWindowHandle trait；Rust 要把 trait 引入作用域才能调用其方法。
            // 教学笔记：[trait 与 impl](../learning-notes/rust-java-differences.md#traits-derive)。
            let main_hwnd = creation_context
                .window_handle()
                .ok()
                // `Option::and_then` 把“可能没有句柄”与后续转换串起来，避免 null 检查。
                // 教学笔记：[Option](../learning-notes/rust-java-differences.md#option)。
                .and_then(|handle| match handle.as_raw() {
                    RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
                    _ => None,
                });

            let mut controller = QrController::new(main_hwnd, &creation_context.egui_ctx);
            if !system_font_found {
                controller.set_status("未找到系统中文字体，请在 Windows 设置中安装中文字体。");
            }
            Ok(Box::new(controller))
        }),
    )
}

/// 从 Windows 字体目录加载中文字体，不将字体文件打包进 exe。
fn install_system_chinese_font(ctx: &eframe::egui::Context) -> bool {
    let windows_dir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let font_dir = windows_dir.join("Fonts");

    // `let-else` 把缺省分支放在前面；它会提前退出当前函数。详见教学笔记：[Option](../learning-notes/rust-java-differences.md#option)。
    let Some(font_data) = ["msyh.ttc", "simhei.ttf", "simsun.ttc"]
        .iter()
        // `find_map` 用闭包尝试每个文件，遇到第一个 Some 就停止。详见教学笔记：[迭代器](../learning-notes/rust-java-differences.md#iterators)。
        .find_map(|file_name| std::fs::read(font_dir.join(file_name)).ok())
        .map(eframe::egui::FontData::from_owned)
    else {
        return false;
    };

    // `Arc::new` 创建线程安全的共享所有权句柄；这里便于 egui 字体定义共享字体数据。
    // 教学笔记：[所有权与 Arc](../learning-notes/rust-java-differences.md#ownership-borrowing)。
    let mut fonts = eframe::egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("system_chinese".to_owned(), Arc::new(font_data));
    fonts
        .families
        .get_mut(&eframe::egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, "system_chinese".to_owned());
    fonts
        .families
        .get_mut(&eframe::egui::FontFamily::Monospace)
        .unwrap()
        .push("system_chinese".to_owned());
    ctx.set_fonts(fonts);
    true
}
