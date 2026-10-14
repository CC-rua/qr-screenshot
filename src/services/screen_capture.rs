use eframe::egui;
use image::RgbaImage;
use screenshots::Screen;

/// 按要求排除并隐藏主窗口，避免截图中包含本程序窗口。
pub fn hide_main_window_for_capture(ctx: &egui::Context, hwnd: Option<isize>) {
    exclude_main_window_from_capture(hwnd);
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
    hide_main_window(hwnd);
}

/// 恢复主窗口、清除截图排除标记并请求焦点。
pub fn restore_main_window(ctx: &egui::Context, hwnd: Option<isize>) {
    ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);

    #[cfg(windows)]
    if let Some(hwnd) = hwnd {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SW_SHOWNORMAL, SetForegroundWindow, SetWindowDisplayAffinity, ShowWindow, WDA_NONE,
        };

        // Win32 是原生 FFI；`unsafe` 标明调用者要保证句柄和 API 用法有效。
        // 教学笔记：[unsafe 与 FFI](../../learning-notes/rust-java-differences.md#unsafe-ffi)。
        unsafe {
            let hwnd = HWND(hwnd);
            let _ = SetWindowDisplayAffinity(hwnd, WDA_NONE);
            let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
            let _ = SetForegroundWindow(hwnd);
        }
    }

    #[cfg(not(windows))]
    let _ = hwnd;
}

/// 等待系统确认主窗口已经隐藏。
pub fn wait_until_hidden(hwnd: Option<isize>) -> bool {
    #[cfg(windows)]
    {
        use std::time::{Duration, Instant};
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::IsWindowVisible;

        // let-else 从 Some 中取出句柄；None 时立即返回，类似对 Optional 的早退检查。
        // 教学笔记：[Option 与 let-else](../../learning-notes/rust-java-differences.md#option)。
        let Some(hwnd) = hwnd else {
            return false;
        };
        let hwnd = HWND(hwnd);
        let deadline = Instant::now() + Duration::from_secs(1);
        while unsafe { IsWindowVisible(hwnd).as_bool() } && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        !unsafe { IsWindowVisible(hwnd).as_bool() }
    }

    #[cfg(not(windows))]
    {
        let _ = hwnd;
        true
    }
}

/// 等待 DWM 提交主窗口隐藏后的桌面合成帧。
pub fn flush_desktop_composition() {
    #[cfg(windows)]
    unsafe {
        let _ = windows::Win32::Graphics::Dwm::DwmFlush();
    }
}

/// 枚举显示器并捕获主显示器。
pub fn capture_primary_screen() -> Result<RgbaImage, String> {
    // `?` 遇到 Err 时从当前函数返回；成功时取出值继续运行，不需要 try/catch。
    // 教学笔记：[Result 与 ?](../../learning-notes/rust-java-differences.md#result-errors)。
    let screens = Screen::all().map_err(|error| error.to_string())?;
    let screen = screens
        .iter()
        .find(|screen| screen.display_info.is_primary)
        .or_else(|| screens.first())
        .ok_or_else(|| "没有找到可用的显示器。".to_owned())?;
    let captured = screen.capture().map_err(|error| error.to_string())?;
    RgbaImage::from_raw(captured.width(), captured.height(), captured.into_raw())
        .ok_or_else(|| "截图像素数据格式无效。".to_owned())
}

// cfg 在编译期选择平台实现，不是运行时 if；非 Windows 构建使用空实现。
// 教学笔记：[条件编译](../../learning-notes/rust-java-differences.md#conditional-compilation)。
#[cfg(windows)]
fn hide_main_window(hwnd: Option<isize>) {
    if let Some(hwnd) = hwnd {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};
        unsafe {
            let _ = ShowWindow(HWND(hwnd), SW_HIDE);
        }
    }
}

#[cfg(not(windows))]
fn hide_main_window(_: Option<isize>) {}

#[cfg(windows)]
fn exclude_main_window_from_capture(hwnd: Option<isize>) {
    if let Some(hwnd) = hwnd {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
        };
        unsafe {
            let _ = SetWindowDisplayAffinity(HWND(hwnd), WDA_EXCLUDEFROMCAPTURE);
        }
    }
}

#[cfg(not(windows))]
fn exclude_main_window_from_capture(_: Option<isize>) {}
