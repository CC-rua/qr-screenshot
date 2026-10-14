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
const CAPTURE_SCAN_ID: &str = "capture-scan";
const EXIT_APP_ID: &str = "exit-app";

/// 创建 Windows 通知区域图标，并将恢复/退出菜单接回 egui 窗口。
///
/// 托盘对象必须保存在控制器里；离开作用域并析构后，Windows 会移除图标。
/// 教学笔记：[闭包与回调所有权](../learning-notes/rust-java-differences.md#closures-move)、[原子请求标记](../learning-notes/rust-java-differences.md#arc-atomicbool)。
pub fn create(
    ctx: egui::Context,
    exit_requested: Arc<AtomicBool>,
    capture_requested: Arc<AtomicBool>,
) -> Result<TrayIcon, String> {
    // 菜单事件使用稳定 ID 分派；显示、扫码和退出都通过同一个原生菜单触发。
    let show_window = MenuItem::with_id(SHOW_WINDOW_ID, "显示主窗口", true, None);
    let capture_scan = MenuItem::with_id(CAPTURE_SCAN_ID, "截图扫码", true, None);
    let exit_app = MenuItem::with_id(EXIT_APP_ID, "退出", true, None);
    let menu = Menu::with_items(&[&show_window, &capture_scan, &exit_app])
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

    // 托盘回调运行在原生事件路径上，不能直接借用 UI 线程里的 QrController。
    // 用共享的 AtomicBool 传递一次“请求截图”的信号，再唤醒 egui 处理它。
    let menu_context = ctx;
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == SHOW_WINDOW_ID {
            restore_window(&menu_context);
        } else if event.id == CAPTURE_SCAN_ID {
            // store 只写入一个布尔标记，不在托盘回调中执行截图或访问 AppModel。
            capture_requested.store(true, Ordering::SeqCst);
            // 显示并请求重绘，确保控制器能在下一帧取走这个信号。
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
