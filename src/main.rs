#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use image::{RgbaImage, imageops};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use screenshots::Screen;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

/// 程序入口：创建 eframe 原生窗口，并把初始化好的字体和 Windows 窗口句柄交给 QrApp。
fn main() -> eframe::Result {
    let options = eframe::NativeOptions::default();

    // run_native 会启动原生事件循环。闭包在窗口创建后运行一次，用于构造 App。
    // 返回的 Result 由 eframe 管理：Err 会结束启动并交还给操作系统调用方。
    eframe::run_native(
        "桌面二维码识别",
        options,
        Box::new(|creation_context| {
            let mut fonts = egui::FontDefinitions::default();
            let system_font_found = if let Some(font_data) = load_system_chinese_font() {
                // 使用 Windows 已安装的中文字体，不把字体文件编进 exe。
                fonts
                    .font_data
                    .insert("system_chinese".to_owned(), Arc::new(font_data));
                fonts
                    .families
                    .get_mut(&egui::FontFamily::Proportional)
                    .unwrap()
                    .insert(0, "system_chinese".to_owned());
                fonts
                    .families
                    .get_mut(&egui::FontFamily::Monospace)
                    .unwrap()
                    .push("system_chinese".to_owned());
                true
            } else {
                false
            };
            creation_context.egui_ctx.set_fonts(fonts);

            // HasWindowHandle 是 raw-window-handle 提供的接口。
            // CreationContext::window_handle() 返回当前 eframe 主窗口的跨平台句柄；
            // as_raw() 将其展开为平台类型，Win32 分支中的 hwnd.get() 得到 Windows HWND 数值。
            // 这里把句柄保存为 isize，之后用 windows crate 调用 Win32 窗口 API。
            let main_hwnd =
                creation_context
                    .window_handle()
                    .ok()
                    .and_then(|handle| match handle.as_raw() {
                        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
                        _ => None,
                    });

            let mut app = QrApp {
                main_hwnd,
                ..QrApp::default()
            };
            if !system_font_found {
                app.status = "未找到系统中文字体，请在 Windows 设置中安装中文字体。".to_owned();
            }
            Ok(Box::new(app))
        }),
    )
}

/// 从 Windows 字体目录读取系统中文字体。只把运行时读到的字节放入 egui，避免将字体嵌入程序。
fn load_system_chinese_font() -> Option<egui::FontData> {
    let windows_dir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let font_dir = windows_dir.join("Fonts");

    // 微软雅黑优先；黑体和宋体作为不同 Windows 安装环境下的备用字体。
    for file_name in ["msyh.ttc", "simhei.ttf", "simsun.ttc"] {
        if let Ok(bytes) = std::fs::read(font_dir.join(file_name)) {
            return Some(egui::FontData::from_owned(bytes));
        }
    }

    None
}

#[derive(Default)]
struct QrApp {
    /// 面向用户的当前状态提示。
    status: String,
    /// 一个或多个二维码解码后的展示文本。
    result_text: String,
    /// Arc 允许主 UI、后台截图线程、选区窗口共享同一状态；Mutex 保证同一时刻只有一方修改。
    overlay: Arc<Mutex<OverlayState>>,
    /// 主窗口的 Win32 HWND。非 Windows 平台或句柄读取失败时为 None。
    main_hwnd: Option<isize>,
}

#[derive(Default)]
struct OverlayState {
    /// 原始全屏截图。Arc 让选区结果把图像交回主窗口时不必复制整张位图。
    screenshot: Option<Arc<RgbaImage>>,
    /// egui 上传到 GPU 后的纹理句柄；None 表示需要从 screenshot 创建纹理。
    texture: Option<egui::TextureHandle>,
    /// true 时选区窗口才绘制画面并接收框选操作。
    active: bool,
    /// 鼠标按下时记录的选区起点，以及拖动时不断更新的终点，坐标单位为 egui 逻辑点。
    selection_start: Option<egui::Pos2>,
    selection_current: Option<egui::Pos2>,
    /// 松开鼠标后的结果：(截图、绘制截图时的屏幕矩形、选区矩形)。
    completed: Option<(Arc<RgbaImage>, egui::Rect, egui::Rect)>,
    /// Esc 取消标记。主窗口读取后会用 take 清零，避免重复处理。
    cancelled: bool,
    /// 后台线程发生的截图错误，交给主 UI 显示。
    capture_error: Option<String>,
}

impl QrApp {
    /// 开始一次“隐藏自身 -> 截图 -> 显示选区”的异步流程。
    fn begin_capture(&mut self, ctx: &egui::Context) {
        self.status = "正在捕获屏幕……".to_owned();
        self.result_text.clear();

        if let Ok(mut overlay) = self.overlay.lock() {
            overlay.screenshot = None;
            overlay.texture = None;
            overlay.active = false;
            overlay.selection_start = None;
            overlay.selection_current = None;
            overlay.completed = None;
            overlay.cancelled = false;
            overlay.capture_error = None;
        }

        // SetWindowDisplayAffinity 告诉 DWM 尽量不要把本窗口放进屏幕捕获结果。
        // 随后同时使用 egui 的窗口命令和 ShowWindow 隐藏主窗：egui 命令进入事件循环队列，
        // ShowWindow 则立即对原生 HWND 生效。这样可避免截图时留下自身窗口的残影。
        exclude_main_window_from_capture(self.main_hwnd);
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        hide_main_window(self.main_hwnd);

        // 后台截图可能耗时；把共享状态和 egui Context 克隆进线程，避免阻塞 UI 绘制。
        let overlay_state = Arc::clone(&self.overlay);
        let repaint_context = ctx.clone();
        let main_hwnd = self.main_hwnd;
        std::thread::spawn(move || {
            // 先确认 Windows 已把 HWND 标记为不可见，再刷新 DWM 合成队列后抓屏。
            // IsWindowVisible 检查的是窗口可见样式；DwmFlush 等待 DWM 提交待处理的桌面合成。
            let capture_result = if wait_until_hidden(main_hwnd) {
                flush_desktop_composition();
                capture_primary_screen()
            } else {
                Err("无法确认主窗口已隐藏，已取消截图以避免截入软件窗口。".to_owned())
            };
            match capture_result {
                Ok(image) => {
                    if let Ok(mut overlay) = overlay_state.lock() {
                        // 将截图交给共享状态。Arc::new 避免后续各 UI 组件复制整张图。
                        overlay.screenshot = Some(Arc::new(image));
                        overlay.texture = None;
                        overlay.active = true;
                    }
                    // 每个 deferred viewport 都用稳定 ID 标识。Visible(true) 和 Focus 是发送给
                    // eframe 的窗口命令：显示预创建的全屏子窗口，并请求它接收键盘/鼠标输入。
                    let overlay_id = egui::ViewportId::from_hash_of("qr-selection-overlay");
                    repaint_context
                        .send_viewport_cmd_to(overlay_id, egui::ViewportCommand::Visible(true));
                    repaint_context.send_viewport_cmd_to(overlay_id, egui::ViewportCommand::Focus);
                }
                Err(error) => {
                    if let Ok(mut overlay) = overlay_state.lock() {
                        overlay.capture_error = Some(error);
                    }
                    // 失败路径也必须恢复主窗，并清除捕获排除标记。
                    restore_main_window(&repaint_context, main_hwnd);
                }
            }
            // 后台线程改了共享状态后主动请求下一帧；否则隐藏的主窗口可能不会及时重绘。
            repaint_context.request_repaint();
        });
    }

    /// 主窗口每帧调用：取走选区窗口交回的结果，并在主线程执行裁剪和解码。
    fn collect_overlay_result(&mut self) {
        let (completed, cancelled, capture_error) = match self.overlay.lock() {
            Ok(mut overlay) => {
                // Option::take 取出值的同时把字段重置为 None，形成一次性消息传递。
                let completed = overlay.completed.take();
                let cancelled = std::mem::take(&mut overlay.cancelled);
                let capture_error = overlay.capture_error.take();
                if completed.is_some() || cancelled || capture_error.is_some() {
                    // 结果已经取走，释放截图纹理和状态中的图像引用。
                    overlay.screenshot = None;
                    overlay.texture = None;
                    overlay.active = false;
                }
                (completed, cancelled, capture_error)
            }
            Err(_) => {
                self.status = "选区窗口状态读取失败。".to_owned();
                return;
            }
        };

        if let Some((screenshot, screen_rect, selection)) = completed {
            // 选区坐标以逻辑点表示；crop_and_decode 会换算成截图像素坐标再处理。
            match crop_and_decode(&screenshot, screen_rect, selection) {
                Ok(contents) if !contents.is_empty() => {
                    self.result_text = contents
                        .iter()
                        .enumerate()
                        .map(|(index, content)| format!("二维码 {}：\n{}", index + 1, content))
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    self.status = format!("识别成功，共找到 {} 个二维码。", contents.len());
                }
                Ok(_) => {
                    self.result_text.clear();
                    self.status =
                        "选区内没有识别到二维码，请尝试框选更大或更清晰的区域。".to_owned();
                }
                Err(error) => {
                    self.result_text.clear();
                    self.status = format!("识别失败：{error}");
                }
            }
        } else if cancelled {
            self.status = "已取消框选。".to_owned();
        } else if let Some(error) = capture_error {
            self.status = format!("屏幕截图失败：{error}");
        }
    }

    /// 声明一个独立的全屏选区 viewport。deferred 表示由 eframe 管理子窗口生命周期，
    /// 回调会在该窗口绘制时运行；隐藏状态下也能先创建它，降低点击后创建/铺满屏幕的延迟。
    fn show_selection_viewport(&self, ctx: &egui::Context) {
        let viewport_id = egui::ViewportId::from_hash_of("qr-selection-overlay");
        let viewport_builder = egui::ViewportBuilder::default()
            .with_title("选择二维码区域")
            .with_fullscreen(true)
            .with_decorations(false)
            .with_resizable(false)
            .with_window_level(egui::WindowLevel::AlwaysOnTop)
            .with_visible(false);
        let overlay_state = Arc::clone(&self.overlay);
        let main_hwnd = self.main_hwnd;

        // 预先创建完整尺寸的隐藏窗口，显示时只需切换可见状态，不再放大主窗口。
        ctx.show_viewport_deferred(viewport_id, viewport_builder, move |ui, _class| {
            draw_selection_overlay(ui, &overlay_state, main_hwnd);
        });
    }
}

impl eframe::App for QrApp {
    /// eframe 每帧调用此方法。按钮、结果区在主窗口绘制，选区 viewport 则单独提交给 eframe。
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.collect_overlay_result();

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("截图识别二维码");
            ui.add_space(12.0);
            ui.label("框选屏幕中的二维码，识别结果会显示在这里。");
            ui.add_space(12.0);

            if ui
                .add_sized([180.0, 42.0], egui::Button::new("框选并识别二维码"))
                .clicked()
            {
                self.begin_capture(ui.ctx());
            }

            ui.add_space(12.0);
            ui.label(&self.status);

            if !self.result_text.is_empty() {
                ui.horizontal(|ui| {
                    ui.label("识别内容");
                    if ui.button("复制内容").clicked() {
                        // egui 将文本复制请求交给宿主平台的系统剪贴板。
                        ui.ctx().copy_text(self.result_text.clone());
                        self.status = "识别内容已复制到剪贴板。".to_owned();
                    }
                });
                ui.add(
                    egui::TextEdit::multiline(&mut self.result_text)
                        .desired_rows(10)
                        .desired_width(f32::INFINITY),
                );
            }
        });

        self.show_selection_viewport(ui.ctx());
    }
}

/// 绘制全屏截图和选区交互。该函数由 eframe 在选区 viewport 的 UI 线程调用。
fn draw_selection_overlay(
    ui: &mut egui::Ui,
    shared: &Arc<Mutex<OverlayState>>,
    main_hwnd: Option<isize>,
) {
    let ctx = ui.ctx().clone();
    let screen_rect = ui.max_rect();
    let mut should_close = None;

    // 持锁期间只读取/更新选区状态并绘制，不在锁内启动耗时截图或解码。
    if let Ok(mut overlay) = shared.lock() {
        if !overlay.active {
            return;
        }
        let Some(screenshot) = overlay.screenshot.clone() else {
            return;
        };

        if overlay.texture.is_none() {
            // ColorImage 是 egui 的 CPU 侧图像格式；from_rgba_unmultiplied 接收宽高和连续 RGBA 字节。
            // load_texture 将 CPU 图像上传为 GPU 纹理，TextureHandle 由状态保存以供后续帧复用。
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [screenshot.width() as usize, screenshot.height() as usize],
                screenshot.as_raw(),
            );
            overlay.texture =
                Some(ctx.load_texture("screen-capture", color_image, egui::TextureOptions::LINEAR));
        }
        let Some(texture) = overlay.texture.clone() else {
            return;
        };

        ui.painter().image(
            texture.id(),
            screen_rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        ui.painter()
            // alpha=110 的黑色覆盖层压暗截图；鼠标选择区域随后再绘制原图以形成“挖空”效果。
            .rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(110));

        // Context::input 在只读输入快照上执行闭包。取出当前指针位置、主键按下/按住/松开和 Esc。
        let (pointer_pos, pressed, down, released, escape_pressed) = ctx.input(|input| {
            (
                input.pointer.interact_pos(),
                input.pointer.primary_pressed(),
                input.pointer.primary_down(),
                input.pointer.primary_released(),
                input.key_pressed(egui::Key::Escape),
            )
        });

        if escape_pressed {
            overlay.cancelled = true;
            should_close = Some(false);
        } else {
            if pressed {
                overlay.selection_start = pointer_pos;
                overlay.selection_current = pointer_pos;
            } else if down {
                if let Some(pointer_pos) = pointer_pos {
                    overlay.selection_current = Some(pointer_pos);
                }
            }

            if let (Some(start), Some(current)) =
                (overlay.selection_start, overlay.selection_current)
            {
                // from_two_pos 自动处理从左上向右下或反方向拖动；intersect 将区域限制在窗口内。
                let selected_rect = egui::Rect::from_two_pos(start, current).intersect(screen_rect);
                // egui 的绘图坐标为逻辑点，纹理 UV 范围是 0..1；把屏幕选区归一化为纹理坐标。
                let u0 = (selected_rect.left() - screen_rect.left()) / screen_rect.width();
                let v0 = (selected_rect.top() - screen_rect.top()) / screen_rect.height();
                let u1 = (selected_rect.right() - screen_rect.left()) / screen_rect.width();
                let v1 = (selected_rect.bottom() - screen_rect.top()) / screen_rect.height();
                ui.painter().image(
                    texture.id(),
                    selected_rect,
                    egui::Rect::from_min_max(egui::pos2(u0, v0), egui::pos2(u1, v1)),
                    egui::Color32::WHITE,
                );
                ui.painter().rect_stroke(
                    selected_rect,
                    0.0,
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(60, 220, 120)),
                    egui::StrokeKind::Inside,
                );
            }

            ui.painter().text(
                egui::pos2(screen_rect.center().x, 36.0),
                egui::Align2::CENTER_TOP,
                "拖动鼠标框选二维码区域 · Esc 取消",
                egui::FontId::proportional(22.0),
                egui::Color32::WHITE,
            );

            if released {
                if let (Some(start), Some(end)) = (overlay.selection_start, pointer_pos) {
                    // 把截图和两个矩形作为一次性完成消息交给主窗口。此处不直接解码，保持窗口回调轻量。
                    overlay.completed = Some((
                        Arc::clone(&screenshot),
                        screen_rect,
                        egui::Rect::from_two_pos(start, end),
                    ));
                    should_close = Some(true);
                }
            }
        }
    }

    if let Some(completed) = should_close {
        // 遮罩结束后先隐藏子 viewport，再显示/聚焦主 viewport 并恢复原生窗口状态。
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        restore_main_window(&ctx, main_hwnd);
        if completed {
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        }
    }
}

#[cfg(windows)]
/// 使用 Win32 ShowWindow 隐藏窗口。SW_HIDE 清除窗口可见状态；返回值表示调用前是否可见，这里无需使用。
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
/// 请求 DWM 在屏幕捕获中排除本程序窗口。此设置只影响捕获呈现，不会隐藏屏幕上的窗口本身。
/// 返回值被忽略：若系统不支持该能力，后续仍会通过隐藏 HWND 和刷新合成继续尝试。
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

#[cfg(windows)]
/// 轮询 IsWindowVisible，最多等待一秒。true 表示系统确认 HWND 已隐藏，false 表示超时或没有句柄。
fn wait_until_hidden(hwnd: Option<isize>) -> bool {
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::IsWindowVisible;

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
fn wait_until_hidden(_: Option<isize>) -> bool {
    true
}

/// 恢复主窗口，并让它重新成为前台窗口。
/// egui viewport 命令负责框架状态，Win32 API 直接恢复 HWND 和焦点；WDA_NONE 撤销截图排除。
fn restore_main_window(ctx: &egui::Context, hwnd: Option<isize>) {
    ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);

    #[cfg(windows)]
    if let Some(hwnd) = hwnd {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SW_SHOWNORMAL, SetForegroundWindow, SetWindowDisplayAffinity, ShowWindow, WDA_NONE,
        };

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

#[cfg(windows)]
/// DwmFlush 是同步 Win32 API：等待当前线程相关的待处理 DWM 桌面合成操作完成。
/// unsafe 是因为 windows crate 将 Win32 FFI 调用标记为需要调用方保证句柄/API 使用正确。
fn flush_desktop_composition() {
    // 等待 DWM 提交隐藏窗口后的桌面帧，防止截图读到窗口消失前的旧画面。
    unsafe {
        let _ = windows::Win32::Graphics::Dwm::DwmFlush();
    }
}

#[cfg(not(windows))]
fn flush_desktop_composition() {}

/// 枚举显示器并捕获主显示器。screenshots 返回自己的 RGBA 像素缓冲；这里重建为项目使用的 image::RgbaImage。
fn capture_primary_screen() -> Result<RgbaImage, String> {
    // Screen::all 返回显示器描述列表；优先 is_primary，若系统没有标记则退回第一台显示器。
    let screens = Screen::all().map_err(|error| error.to_string())?;
    let screen = screens
        .iter()
        .find(|screen| screen.display_info.is_primary)
        .or_else(|| screens.first())
        .ok_or_else(|| "没有找到可用的显示器。".to_owned())?;
    // Screen::capture 返回截图对象，into_raw 移出连续像素数据；from_raw 检查宽高与数据长度是否匹配。
    let captured = screen.capture().map_err(|error| error.to_string())?;
    RgbaImage::from_raw(captured.width(), captured.height(), captured.into_raw())
        .ok_or_else(|| "截图像素数据格式无效。".to_owned())
}

/// 将屏幕逻辑坐标的选区换算成像素，裁剪后交给 rqrr 检测并解码二维码。
fn crop_and_decode(
    screenshot: &RgbaImage,
    screen_rect: egui::Rect,
    selection: egui::Rect,
) -> Result<Vec<String>, String> {
    let selection = selection.intersect(screen_rect);
    if selection.width() < 8.0 || selection.height() < 8.0 {
        return Err("选区太小，请重新框选。".to_owned());
    }

    // 以截图像素尺寸除以绘制区域逻辑尺寸，分别得到 X/Y 缩放比例，兼容系统显示缩放。
    let scale_x = screenshot.width() as f32 / screen_rect.width();
    let scale_y = screenshot.height() as f32 / screen_rect.height();
    // floor/ceil 覆盖选区的完整像素范围；随后钳制到图像边界，避免越界裁剪。
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

    // crop_imm 以不可变方式裁图；to_image 生成独立像素图，供灰度转换和解码使用。
    let crop = imageops::crop_imm(screenshot, x0, y0, x1 - x0, y1 - y0).to_image();
    let grayscale = imageops::grayscale(&crop);
    // PreparedImage 是 rqrr 的预处理图像。detect_grids 找出二维码候选区域；
    // 每个 Grid::decode 返回定位信息和内容。filter_map 跳过解码失败的码，只收集成功内容。
    let mut prepared = rqrr::PreparedImage::prepare(grayscale);
    let contents = prepared
        .detect_grids()
        .into_iter()
        .filter_map(|grid| grid.decode().ok().map(|(_, content)| content))
        .collect();
    Ok(contents)
}
