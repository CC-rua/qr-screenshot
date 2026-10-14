use eframe::egui;
use image::RgbaImage;
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

/// 主窗口当前显示的工作区。
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum MainTab {
    #[default]
    Scan,
    Generate,
}

/// 主窗口需要展示的数据与应用运行状态。
// AppModel 用 struct 保存状态；derive 由编译器生成 Default trait 实现，不是 Java 运行时注解。
// 教学笔记：[struct、impl 与 derive](../learning-notes/rust-java-differences.md#struct-impl-methods)。
#[derive(Default)]
pub struct AppModel {
    // 默认私有；`pub(crate)` 只开放给当前 crate，作用域不等同 Java package。
    // 教学笔记：[方法与可见性](../learning-notes/rust-java-differences.md#struct-impl-methods)。
    pub(crate) status: String,
    pub(crate) results: Vec<String>,
    pub(crate) active_tab: MainTab,
    /// 复制反馈只在对应结果卡上短暂显示。
    pub(crate) copied_result: Option<usize>,
    pub(crate) copy_feedback_until: Option<Instant>,
    /// 用户输入的二维码文本和 PNG 保存路径。
    pub(crate) qr_text: String,
    /// 保留生成图像用于保存，同时保留 egui 纹理用于界面预览。
    pub(crate) generated_qr: Option<image::GrayImage>,
    pub(crate) generated_qr_texture: Option<egui::TextureHandle>,
    // `Arc<Mutex<T>>` 让多个线程共享数据，并通过锁串行修改。
    // 教学笔记：[Arc 与 Mutex](../learning-notes/rust-java-differences.md#arc-mutex)。
    pub(crate) overlay: Arc<Mutex<OverlayState>>,
}

/// 截图选区窗口与主窗口之间共享的一次性消息和视图状态。
// AppModel 用 struct 保存状态；derive 由编译器生成 Default trait 实现，不是 Java 运行时注解。
// 教学笔记：[struct、impl 与 derive](../learning-notes/rust-java-differences.md#struct-impl-methods)。
#[derive(Default)]
pub(crate) struct OverlayState {
    // `Option<T>` 显式表达“尚无截图”；Rust 通常用它替代可空引用。
    // 教学笔记：[Option](../learning-notes/rust-java-differences.md#option)。
    pub(crate) screenshot: Option<Arc<RgbaImage>>,
    pub(crate) texture: Option<egui::TextureHandle>,
    pub(crate) active: bool,
    pub(crate) selection_start: Option<egui::Pos2>,
    pub(crate) selection_current: Option<egui::Pos2>,
    pub(crate) completed: Option<(Arc<RgbaImage>, egui::Rect, egui::Rect)>,
    pub(crate) cancelled: bool,
    pub(crate) capture_error: Option<String>,
}

impl OverlayState {
    /// 清理上一轮选区及截图状态，准备新一轮捕获。
    // `&mut self` 表示借用并修改当前值，不转移对象所有权。
    // 教学笔记：[方法接收者](../learning-notes/rust-java-differences.md#struct-impl-methods)。
    pub(crate) fn reset(&mut self) {
        self.screenshot = None;
        self.texture = None;
        self.active = false;
        self.selection_start = None;
        self.selection_current = None;
        self.completed = None;
        self.cancelled = false;
        self.capture_error = None;
    }
}
