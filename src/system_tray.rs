use eframe::egui;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

const SHOW_WINDOW_ID: &str = "show-window";
const EXIT_APP_ID: &str = "exit-app";

/// 创建 Windows 通知区域图标，并将恢复/退出菜单接回 egui 窗口。
///
/// 托盘对象必须保存在控制器里；离开作用域并析构后，Windows 会移除图标。
/// 教学笔记：[闭包、Arc 与回调所有权](../learning-notes/rust-java-differences.md#closures-move)。
pub fn create(ctx: egui::Context, exit_requested: Arc<AtomicBool>) -> Result<TrayIcon, String> {
    let show_window = MenuItem::with_id(SHOW_WINDOW_ID, "显示主窗口", true, None);
    let exit_app = MenuItem::with_id(EXIT_APP_ID, "退出", true, None);
    let menu = Menu::with_items(&[&show_window, &exit_app])
        .map_err(|error| format!("创建托盘菜单失败：{error}"))?;

    let icon = Icon::from_rgba(crate::app_icon::rgba(32), 32, 32)
        .map_err(|error| format!("创建托盘图标失败：{error}"))?;
    let tray_icon = TrayIconBuilder::new()
        .with_tooltip("桌面二维码识别")
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .build()
        .map_err(|error| format!("创建托盘图标失败：{error}"))?;

    // 托盘回调由原生窗口事件循环触发。egui::Context 可跨回调持有，并把窗口命令排入 UI。
    let restore_context = ctx.clone();
    tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
        let should_restore = match event {
            tray_icon::TrayIconEvent::DoubleClick { .. }
            | tray_icon::TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } => true,
            // 库将事件枚举标记为 non_exhaustive，未来可能追加新事件，因此保留兜底分支。
            _ => false,
        };
        if should_restore {
            restore_window(&restore_context);
        }
    }));

    let menu_context = ctx;
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == SHOW_WINDOW_ID {
            restore_window(&menu_context);
        } else if event.id == EXIT_APP_ID {
            // 退出菜单先标记原因，再发送关闭命令；控制器据此允许真正退出。
            exit_requested.store(true, Ordering::SeqCst);
            menu_context.send_viewport_cmd(egui::ViewportCommand::Close);
            menu_context.request_repaint();
        }
    }));

    Ok(tray_icon)
}

fn restore_window(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    ctx.request_repaint();
}
