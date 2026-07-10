use crate::os::input_hook::{self, KeyAction};
use crate::os::monitor::DialogInfo;
use crate::os::window_ext;
use crate::ui::window::FrameIntents;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

const HIDE_GRACE_MS: u64 = 120;
const UI_TICK_MS: u64 = 30;
/// 悬浮条逻辑高度（像素，按对话框 DPI 缩放为物理像素）。
const OVERLAY_HEIGHT_LOGICAL: u32 = 140;
/// 悬浮条与对话框下边缘的物理像素间距（0 = 紧贴）。
const OVERLAY_GAP: i32 = 0;

/// 从实现了 `HasWindowHandle` 的对象（CreationContext / Frame）中取 Win32 HWND。
fn extract_hwnd(handle: &impl HasWindowHandle) -> isize {
    match handle.window_handle() {
        Ok(h) => match h.as_raw() {
            RawWindowHandle::Win32(win32) => win32.hwnd.get(),
            _ => 0,
        },
        Err(_) => 0,
    }
}

pub struct PathWarpApp {
    /// 悬浮窗自身的 HWND（首帧从 eframe Frame 获取，0 表示尚未就绪）。
    overlay_hwnd: isize,
    styles_applied: bool,

    pub dialog_rx: Receiver<Option<DialogInfo>>,
    /// 低层键盘钩子送来的输入意图（悬浮条为非激活窗，无法用 egui 接收键盘）。
    key_rx: Receiver<KeyAction>,
    pub target_dialog: Option<DialogInfo>,
    pub pending_none_since: Option<Instant>,

    pub paths: Vec<String>,
    pub search_query: String,
    pub selected_index: usize,

    /// 用户按 ESC 主动隐藏时记录被抑制的对话框，避免同一会话立即被重新拉起。
    user_hidden_dialog_hwnd: Option<isize>,
    last_applied_visible: Option<bool>,
    last_applied_dialog: Option<DialogInfo>,
}

impl PathWarpApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        dialog_rx: Receiver<Option<DialogInfo>>,
        key_rx: Receiver<KeyAction>,
    ) -> Self {
        Self {
            overlay_hwnd: extract_hwnd(cc),
            styles_applied: false,

            dialog_rx,
            key_rx,
            target_dialog: None,
            pending_none_since: None,

            paths: crate::os::explorer::get_open_windows(),
            search_query: String::new(),
            selected_index: 0,

            user_hidden_dialog_hwnd: None,
            last_applied_visible: None,
            last_applied_dialog: None,
        }
    }

    /// 首帧确保拿到 HWND 并应用非激活扩展样式（幂等）。
    fn ensure_overlay_window(&mut self, frame: &eframe::Frame) {
        if self.overlay_hwnd == 0 {
            self.overlay_hwnd = extract_hwnd(frame);
        }
        if self.overlay_hwnd != 0 && !self.styles_applied {
            self.styles_applied = window_ext::apply_overlay_ex_styles(self.overlay_hwnd);
            if self.styles_applied {
                log::debug!("[overlay] applied non-activating ex-styles to hwnd={}", self.overlay_hwnd);
            }
        }
    }

    fn set_overlay_visible(&mut self, visible: bool) {
        if self.last_applied_visible == Some(visible) {
            return;
        }
        self.last_applied_visible = Some(visible);
        if !visible {
            window_ext::hide(self.overlay_hwnd);
            self.last_applied_dialog = None;
            log::debug!("[overlay] hidden");
        }
        // 显示动作由 place_overlay_for_dialog 的 dock() 完成（显示+定位一次到位）。
    }

    pub fn hide_overlay_by_user(&mut self) {
        self.user_hidden_dialog_hwnd = self.target_dialog.map(|d| d.hwnd);
        log::debug!("[overlay] user pressed ESC, suppress current dialog session");
        self.target_dialog = None;
        self.pending_none_since = None;
        self.search_query.clear();
        self.selected_index = 0;
        self.set_overlay_visible(false);
    }

    fn hide_overlay_by_system(&mut self) {
        self.target_dialog = None;
        self.pending_none_since = None;
        self.search_query.clear();
        self.selected_index = 0;
        self.set_overlay_visible(false);
    }

    /// 排空键盘钩子通道，直接消费文本编辑类动作，返回本帧的导航/确认/退出意图。
    fn drain_key_actions(&mut self) -> (FrameIntents, bool) {
        let mut intents = FrameIntents::default();
        let mut escape = false;
        for action in self.key_rx.try_iter() {
            match action {
                KeyAction::Char(c) => self.search_query.push(c),
                KeyAction::Backspace => {
                    self.search_query.pop();
                }
                KeyAction::Up => intents.nav_up = true,
                KeyAction::Down => intents.nav_down = true,
                KeyAction::Enter => intents.confirm = true,
                KeyAction::Escape => escape = true,
            }
        }
        (intents, escape)
    }

    /// 以物理像素把悬浮条停靠到对话框正下方，并显示（不抢焦点）。
    fn place_overlay_for_dialog(&mut self, dialog: DialogInfo) {
        let needs_update =
            self.last_applied_dialog != Some(dialog) || self.last_applied_visible != Some(true);
        if !needs_update {
            return;
        }

        let scaled_height = OVERLAY_HEIGHT_LOGICAL.saturating_mul(dialog.dpi) / 96;
        let height = i32::try_from(scaled_height).unwrap_or(OVERLAY_HEIGHT_LOGICAL as i32);

        let x = dialog.x;
        let y = dialog.y + dialog.height + OVERLAY_GAP;
        let width = dialog.width;

        log::trace!("[overlay] dock at ({}, {}) {}x{}", x, y, width, height);
        window_ext::dock(self.overlay_hwnd, x, y, width, height);

        self.last_applied_dialog = Some(dialog);
        self.last_applied_visible = Some(true);
    }

    fn sync_dialog_state_from_channel(&mut self, ctx: &eframe::egui::Context) {
        let mut newest_some: Option<DialogInfo> = None;
        let mut saw_none = false;

        for msg in self.dialog_rx.try_iter() {
            match msg {
                Some(info) => newest_some = Some(info),
                None => saw_none = true,
            }
        }

        if let Some(info) = newest_some {
            if self.user_hidden_dialog_hwnd == Some(info.hwnd) {
                self.pending_none_since = None;
                self.target_dialog = None;
                return;
            }

            if self.user_hidden_dialog_hwnd.is_some() {
                log::debug!(
                    "[overlay] release user suppression due to dialog switch: {:?} -> {}",
                    self.user_hidden_dialog_hwnd,
                    info.hwnd
                );
                self.user_hidden_dialog_hwnd = None;
            }

            self.target_dialog = Some(info);
            self.pending_none_since = None;
            self.paths = crate::os::explorer::get_open_windows();
            return;
        }

        if saw_none {
            match self.pending_none_since {
                None => {
                    self.pending_none_since = Some(Instant::now());
                    ctx.request_repaint_after(Duration::from_millis(UI_TICK_MS));
                }
                Some(since) => {
                    if Instant::now().duration_since(since) >= Duration::from_millis(HIDE_GRACE_MS) {
                        if self.user_hidden_dialog_hwnd.take().is_some() {
                            log::debug!("[overlay] release user suppression after dialog session ended");
                        }
                        self.hide_overlay_by_system();
                    } else {
                        ctx.request_repaint_after(Duration::from_millis(UI_TICK_MS));
                    }
                }
            }
        }

        // 即使 None 只到达一次，也要在宽限超时后收敛隐藏。
        if let Some(since) = self.pending_none_since
            && Instant::now().duration_since(since) >= Duration::from_millis(HIDE_GRACE_MS)
        {
            if self.user_hidden_dialog_hwnd.take().is_some() {
                log::debug!("[overlay] release user suppression after grace timeout");
            }
            self.hide_overlay_by_system();
        }
    }
}

impl eframe::App for PathWarpApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // 透明背景，让悬浮卡片的圆角与阴影落在透明区域上。
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        self.ensure_overlay_window(frame);
        self.sync_dialog_state_from_channel(ctx);

        // 唯一显隐门控：目标对话框仍是前台窗口时才显示。
        // 悬浮窗为非激活窗口，点击它不会改变前台，故此判定在交互期间保持为真。
        let active = self
            .target_dialog
            .is_some_and(|d| crate::os::monitor::is_foreground_hwnd(d.hwnd));

        // 通知键盘钩子：仅在悬浮条可见时截获打字/导航键，否则全部透传给对话框。
        input_hook::set_active(active);

        if active {
            let (intents, escape) = self.drain_key_actions();
            if escape {
                self.hide_overlay_by_user();
                input_hook::set_active(false);
                egui::CentralPanel::default().show(ctx, |_| {});
            } else if let Some(dialog) = self.target_dialog {
                self.place_overlay_for_dialog(dialog);
                crate::ui::window::render(ctx, self, intents);
            }
        } else {
            self.set_overlay_visible(false);
            egui::CentralPanel::default().show(ctx, |_| {});
        }

        // 跟踪期间持续重绘以平滑跟随对话框移动；空闲时不重绘以省电。
        if self.target_dialog.is_some() || self.pending_none_since.is_some() {
            ctx.request_repaint();
        }
    }
}
