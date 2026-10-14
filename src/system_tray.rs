//! Lightweight Windows tray host. The eframe UI runs in a separate process on demand.
use std::{
    process::{Child, Command},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use tray_icon::{
    Icon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, GetLastError, HANDLE, HWND, LPARAM, WAIT_OBJECT_0, WPARAM},
        System::Threading::{
            CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, OpenEventW,
            SYNCHRONIZATION_SYNCHRONIZE, SetEvent, WaitForSingleObject,
        },
        UI::WindowsAndMessaging::{
            DispatchMessageW, FindWindowW, GetMessageW, MSG, PostMessageW, PostQuitMessage,
            SW_RESTORE, SetForegroundWindow, ShowWindow, TranslateMessage, WM_CLOSE,
        },
    },
    core::PCWSTR,
};

const HOST_MUTEX: &str = "Local\\qr-screenshot-tray-host";
const UI_MUTEX: &str = "Local\\qr-screenshot-ui";
const CAPTURE_EVENT: &str = "Local\\qr-screenshot-capture";
const WINDOW_TITLE: &str = "桌面二维码识别";
const SHOW_WINDOW_ID: &str = "show-window";
const CAPTURE_SCAN_ID: &str = "capture-scan";
const EXIT_APP_ID: &str = "exit-app";

/// Run the lightweight tray host; the eframe UI starts only after a user action.
pub fn run_host() -> Result<(), String> {
    let (_host_mutex, host_already_running) = create_named_mutex(HOST_MUTEX)?;
    if host_already_running {
        start_or_activate_ui(None, false);
        return Ok(());
    }

    let capture_event = unsafe {
        CreateEventW(None, false, false, wide(CAPTURE_EVENT).as_pcwstr())
            .map_err(|error| format!("创建截图请求事件失败：{error}"))?
    };
    let _capture_event = OwnedHandle(capture_event);
    let children = Arc::new(Mutex::new(None::<Child>));

    let menu = Menu::with_items(&[
        &MenuItem::with_id(SHOW_WINDOW_ID, "显示主窗口", true, None),
        &MenuItem::with_id(CAPTURE_SCAN_ID, "截图扫码", true, None),
        &MenuItem::with_id(EXIT_APP_ID, "退出", true, None),
    ])
    .map_err(|error| format!("创建托盘菜单失败：{error}"))?;
    let icon = Icon::from_rgba(crate::app_icon::tray_rgba(), 32, 32)
        .map_err(|error| format!("创建托盘图标失败：{error}"))?;
    let tray_icon = TrayIconBuilder::new()
        .with_tooltip("桌面二维码识别")
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .build()
        .map_err(|error| format!("创建系统托盘图标失败：{error}"))?;

    register_events(Arc::clone(&children));
    start_or_activate_ui(Some(&children), false);
    message_loop();

    if let Ok(mut child) = children.lock() {
        stop_child(&mut child);
    }
    drop(tray_icon);
    Ok(())
}

/// Hold this mutex for the lifetime of the active eframe window process.
pub fn acquire_ui_instance() -> Result<(OwnedHandle, bool), String> {
    create_named_mutex(UI_MUTEX)
}

/// Open the host's auto-reset event so the controller can handle tray scan requests.
pub fn open_capture_event() -> Option<OwnedHandle> {
    unsafe {
        OpenEventW(
            SYNCHRONIZATION_SYNCHRONIZE,
            false,
            wide(CAPTURE_EVENT).as_pcwstr(),
        )
        .ok()
        .map(OwnedHandle)
    }
}

pub fn activate_existing_ui(capture: bool) {
    if capture {
        signal_capture_event();
    }
    let _ = restore_window();
}

pub fn take_capture_request(event: Option<&OwnedHandle>) -> bool {
    let Some(event) = event else {
        return false;
    };
    unsafe { WaitForSingleObject(event.0, 0) == WAIT_OBJECT_0 }
}

fn register_events(children: Arc<Mutex<Option<Child>>>) {
    let tray_children = Arc::clone(&children);
    TrayIconEvent::set_event_handler(Some(move |event| {
        if matches!(
            event,
            TrayIconEvent::DoubleClick { .. }
                | TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                }
        ) {
            start_or_activate_ui(Some(&tray_children), false);
        }
    }));

    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == SHOW_WINDOW_ID {
            start_or_activate_ui(Some(&children), false);
        } else if event.id == CAPTURE_SCAN_ID {
            start_or_activate_ui(Some(&children), true);
        } else if event.id == EXIT_APP_ID {
            unsafe { PostQuitMessage(0) };
        }
    }));
}

fn start_or_activate_ui(children: Option<&Arc<Mutex<Option<Child>>>>, capture: bool) {
    if restore_window() {
        if capture {
            signal_capture_event();
        }
        return;
    }

    if let Some(children) = children {
        let Ok(mut child) = children.lock() else {
            return;
        };
        if let Some(running) = child.as_mut() {
            if running.try_wait().ok().flatten().is_none() {
                if capture {
                    signal_capture_event();
                }
                return;
            }
            *child = None;
        }
        *child = spawn_ui(capture).ok();
    } else {
        if capture {
            signal_capture_event();
        }
        let _ = spawn_ui(capture);
    }
}

fn spawn_ui(capture: bool) -> Result<Child, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let mut command = Command::new(executable);
    command.arg("--ui");
    if capture {
        command.arg("--capture");
    }
    command
        .spawn()
        .map_err(|error| format!("启动主窗口失败：{error}"))
}

fn restore_window() -> bool {
    let title = wide(WINDOW_TITLE);
    let mut handle = HWND(0);
    for _ in 0..20 {
        handle = unsafe { FindWindowW(PCWSTR::null(), title.as_pcwstr()) };
        if handle.0 != 0 {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    if handle.0 == 0 {
        return false;
    }
    unsafe {
        let _ = ShowWindow(handle, SW_RESTORE);
        let _ = SetForegroundWindow(handle);
    }
    true
}

fn signal_capture_event() {
    let Ok(event) =
        (unsafe { OpenEventW(EVENT_MODIFY_STATE, false, wide(CAPTURE_EVENT).as_pcwstr()) })
    else {
        return;
    };
    let event = OwnedHandle(event);
    unsafe {
        let _ = SetEvent(event.0);
    }
}

fn message_loop() {
    loop {
        let mut message = MSG::default();
        let result = unsafe { GetMessageW(&mut message, HWND(0), 0, 0) }.0;
        if result <= 0 {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&message);
            let _ = DispatchMessageW(&message);
        }
    }
}

fn stop_child(child: &mut Option<Child>) {
    if let Some(mut child) = child.take() {
        if child.try_wait().ok().flatten().is_none() {
            if let Some(hwnd) = find_window() {
                unsafe {
                    let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
                }
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn find_window() -> Option<HWND> {
    let title = wide(WINDOW_TITLE);
    let hwnd = unsafe { FindWindowW(PCWSTR::null(), title.as_pcwstr()) };
    (hwnd.0 != 0).then_some(hwnd)
}

fn create_named_mutex(name: &str) -> Result<(OwnedHandle, bool), String> {
    let name = wide(name);
    let handle = unsafe {
        CreateMutexW(None, false, name.as_pcwstr())
            .map_err(|error| format!("创建单实例标记失败：{error}"))?
    };
    let error = unsafe { GetLastError() }
        .err()
        .map(|error| error.code().0 as u32);
    Ok((OwnedHandle(handle), error == Some(0x8007_00B7)))
}

struct WideString(Vec<u16>);

impl WideString {
    fn as_pcwstr(&self) -> PCWSTR {
        PCWSTR(self.0.as_ptr())
    }
}

fn wide(text: &str) -> WideString {
    WideString(text.encode_utf16().chain(Some(0)).collect())
}

pub struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
