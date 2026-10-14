use eframe::egui;
use std::sync::Arc;

use crate::{
    model::{AppModel, MainTab},
    services::{qr_decoder, qr_generator, screen_capture},
    theme,
    view::{self, MainAction, OverlayAction},
};

/// 应用控制器：连接视图动作、应用模型和截图/解码服务。
pub struct QrController {
    model: AppModel,
    main_hwnd: Option<isize>,
    #[cfg(windows)]
    capture_event: Option<crate::system_tray::OwnedHandle>,
    #[cfg(windows)]
    start_capture: bool,
}

impl QrController {
    pub fn new(
        main_hwnd: Option<isize>,
        ctx: &egui::Context,
        #[cfg(windows)] capture_event: Option<crate::system_tray::OwnedHandle>,
        #[cfg(windows)] start_capture: bool,
    ) -> Self {
        theme::apply(ctx);

        Self {
            model: AppModel::default(),
            main_hwnd,
            #[cfg(windows)]
            capture_event,
            #[cfg(windows)]
            start_capture,
        }
    }

    pub fn set_status(&mut self, status: &str) {
        self.model.status = status.to_owned();
    }

    /// 隐藏当前窗口、截图，再用当前窗口显示选区遮罩。
    fn begin_capture(&mut self, ctx: &egui::Context) {
        self.model.status = "正在捕获屏幕……".to_owned();
        self.model.results.clear();

        // `if let` 只处理加锁成功的一种模式；`&mut` 借用让我们在锁保护下修改共享状态。
        // 教学笔记：[模式匹配与借用](../learning-notes/rust-java-differences.md#option)。
        if let Ok(mut overlay) = self.model.overlay.lock() {
            overlay.reset();
        }

        screen_capture::hide_main_window_for_capture(ctx, self.main_hwnd);
        let overlay_state = Arc::clone(&self.model.overlay);
        let repaint_context = ctx.clone();
        let main_hwnd = self.main_hwnd;
        // `move` 将闭包用到的共享句柄、上下文和句柄值带进新线程，避免借用当前栈帧。
        // 教学笔记：[闭包与 move](../learning-notes/rust-java-differences.md#closures-move)。
        std::thread::spawn(move || {
            let capture_result = if screen_capture::wait_until_hidden(main_hwnd) {
                screen_capture::flush_desktop_composition();
                screen_capture::capture_primary_screen()
            } else {
                Err("无法确认主窗口已隐藏，已取消截图以避免截入软件窗口。".to_owned())
            };

            // `Result` 用 Ok/Err 表示成功或可恢复失败；`match` 要覆盖两种分支。
            // 教学笔记：[Result 与错误处理](../learning-notes/rust-java-differences.md#result-errors)。
            match capture_result {
                Ok(image) => {
                    if let Ok(mut overlay) = overlay_state.lock() {
                        overlay.screenshot = Some(Arc::new(image));
                        overlay.texture = None;
                        overlay.active = true;
                    }
                    repaint_context.send_viewport_cmd_to(
                        egui::ViewportId::ROOT,
                        egui::ViewportCommand::Decorations(false),
                    );
                    repaint_context.send_viewport_cmd_to(
                        egui::ViewportId::ROOT,
                        egui::ViewportCommand::Fullscreen(true),
                    );
                    repaint_context.send_viewport_cmd_to(
                        egui::ViewportId::ROOT,
                        egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop),
                    );
                    repaint_context.send_viewport_cmd_to(
                        egui::ViewportId::ROOT,
                        egui::ViewportCommand::Visible(true),
                    );
                    repaint_context
                        .send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
                }
                Err(error) => {
                    if let Ok(mut overlay) = overlay_state.lock() {
                        overlay.capture_error = Some(error);
                    }
                    screen_capture::restore_main_window(&repaint_context, main_hwnd);
                }
            }
            repaint_context.request_repaint();
        });
    }

    /// 读取选区遮罩送回的一次性事件，并在主线程中执行解码。
    fn collect_overlay_result(&mut self) {
        let (completed, cancelled, capture_error) = match self.model.overlay.lock() {
            Ok(mut overlay) => {
                // `Option::take` 取出所有权并把字段重置为 None，形成一次性消息。
                // 教学笔记：[Option](../learning-notes/rust-java-differences.md#option)。
                let completed = overlay.completed.take();
                let cancelled = std::mem::take(&mut overlay.cancelled);
                let capture_error = overlay.capture_error.take();
                if completed.is_some() || cancelled || capture_error.is_some() {
                    overlay.screenshot = None;
                    overlay.texture = None;
                    overlay.active = false;
                }
                (completed, cancelled, capture_error)
            }
            Err(_) => {
                self.model.status = "选区状态读取失败。".to_owned();
                return;
            }
        };

        if let Some((screenshot, screen_rect, selection)) = completed {
            // 解码返回 Result；成功、未识别和失败由不同分支更新模型。
            // 教学笔记：[Result 与 match](../learning-notes/rust-java-differences.md#result-errors)。
            match qr_decoder::crop_and_decode(&screenshot, screen_rect, selection) {
                Ok(contents) if !contents.is_empty() => {
                    let count = contents.len();
                    self.model.results = contents;
                    self.model.status = format!("识别成功，共找到 {} 个二维码。", count);
                }
                Ok(_) => {
                    self.model.results.clear();
                    self.model.status =
                        "选区内没有识别到二维码，请尝试框选更大或更清晰的区域。".to_owned();
                }
                Err(error) => {
                    self.model.results.clear();
                    self.model.status = format!("识别失败：{error}");
                }
            }
        } else if cancelled {
            self.model.status = "已取消框选。".to_owned();
        } else if let Some(error) = capture_error {
            self.model.status = format!("屏幕截图失败：{error}");
        }
    }

    // `match` 拆解带数据的枚举；CopyResult 携带结果索引。
    // 教学笔记：[枚举与 match](../learning-notes/rust-java-differences.md#enum-match)。
    fn handle_main_action(&mut self, action: MainAction, ctx: &egui::Context) {
        match action {
            MainAction::BeginCapture => self.begin_capture(ctx),
            MainAction::GenerateQr => match qr_generator::generate(&self.model.qr_text) {
                Ok(image) => {
                    let color_image = egui::ColorImage::from_gray(
                        [image.width() as usize, image.height() as usize],
                        image.as_raw(),
                    );
                    self.model.generated_qr_texture = Some(ctx.load_texture(
                        "generated-qr-code",
                        color_image,
                        egui::TextureOptions::NEAREST,
                    ));
                    self.model.generated_qr = Some(image);
                    self.model.status = "二维码已生成，可预览或保存为 PNG。".to_owned();
                }
                Err(error) => {
                    self.model.generated_qr = None;
                    self.model.generated_qr_texture = None;
                    self.model.status = error;
                }
            },
            MainAction::SaveQr => self.save_qr_image(),
            MainAction::CopyResult(index) => {
                // get 返回 Option<&String>，索引越界时不会 panic，也不需要 null。
                // 教学笔记：[Option 与借用](../learning-notes/rust-java-differences.md#option)。
                if let Some(content) = self.model.results.get(index) {
                    ctx.copy_text(content.clone());
                    self.model.status = format!("二维码 {} 的内容已复制到剪贴板。", index + 1);
                }
            }
        }
    }

    /// 打开 Windows 原生“另存为”窗口；用户取消时 save_file 返回 None，不写入文件。
    /// 教学笔记：[Option 表示可取消结果](../learning-notes/rust-java-differences.md#option)。
    #[cfg(windows)]
    fn save_qr_image(&mut self) {
        let Some(image) = self.model.generated_qr.as_ref() else {
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .set_title("保存二维码图片")
            .set_file_name("qrcode.png")
            .add_filter("PNG 图片", &["png"])
            .save_file()
        else {
            return;
        };

        match qr_generator::save_png(image, &path) {
            Ok(()) => self.model.status = format!("二维码 PNG 已保存到：{}", path.display()),
            Err(error) => self.model.status = error,
        }
    }

    #[cfg(not(windows))]
    fn save_qr_image(&mut self) {
        self.model.status = "文件另存为窗口目前仅在 Windows 上提供。".to_owned();
    }

    fn finish_selection_viewport(
        ctx: &egui::Context,
        main_hwnd: Option<isize>,
        action: OverlayAction,
    ) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
            egui::WindowLevel::Normal,
        ));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(460.0, 600.0)));
        screen_capture::restore_main_window(ctx, main_hwnd);
        if matches!(action, OverlayAction::Completed) {
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        }
    }
}

// 实现 eframe 的 App trait，类似实现 Java interface；trait 规定框架调用的入口方法。
// 教学笔记：[trait 与 impl](../learning-notes/rust-java-differences.md#traits-derive)。
impl eframe::App for QrController {
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.collect_overlay_result();
        let ctx = _ui.ctx().clone();

        // 主窗口最小化时结束 UI 进程，交给常驻托盘宿主按需重新启动。
        #[cfg(windows)]
        if ctx.input(|input| input.viewport().minimized.unwrap_or(false)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        #[cfg(windows)]
        if std::mem::take(&mut self.start_capture) {
            self.begin_capture(&ctx);
        }

        #[cfg(windows)]
        if crate::system_tray::take_capture_request(self.capture_event.as_ref()) {
            self.model.active_tab = MainTab::Scan;
            self.begin_capture(&ctx);
        }

        let overlay_action = self
            .model
            .overlay
            .lock()
            .ok()
            .and_then(|mut overlay| view::draw_selection_overlay(_ui, &mut overlay));
        if let Some(action) = overlay_action {
            Self::finish_selection_viewport(&ctx, self.main_hwnd, action);
        } else if !self
            .model
            .overlay
            .lock()
            .is_ok_and(|overlay| overlay.active)
        {
            if let Some(action) = view::draw_main(&mut self.model, _ui) {
                self.handle_main_action(action, &ctx);
            }
        }
    }
}
