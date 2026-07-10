//! eframe 应用外壳：把外部事件（对话框通道、键盘钩子、egui 鼠标响应）喂给纯
//! 控制器 [`crate::core::controller::Controller`]，并执行控制器返回的 [`Effect`]。
//! 本文件不含任何显隐/停靠/注入/去抖判断——那些全在控制器里，可被单测覆盖。

use crate::core::controller::{Controller, Effect, Env, Event};
use crate::os::input_hook::{self, KeyAction};
use crate::os::monitor::{self, DialogInfo};
use crate::os::{explorer, window_ext};
use crate::ui::window::UiEvent;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

/// 跟踪期的重绘心跳周期：推进去抖/宽限计时并平滑跟随对话框移动。
const UI_TICK_MS: u64 = 30;

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

    dialog_rx: Receiver<Option<DialogInfo>>,
    /// 低层键盘钩子送来的输入意图（悬浮条为非激活窗，无法用 egui 接收键盘）。
    key_rx: Receiver<KeyAction>,

    controller: Controller,
}

impl PathWarpApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        dialog_rx: Receiver<Option<DialogInfo>>,
        key_rx: Receiver<KeyAction>,
    ) -> Self {
        let mut app = Self {
            overlay_hwnd: extract_hwnd(cc),
            styles_applied: false,
            dialog_rx,
            key_rx,
            controller: Controller::new(),
        };
        // 尽早应用非激活样式并停靠到屏幕外，避免启动时窗口在默认位置可见。
        app.ensure_overlay_window_by_hwnd(app.overlay_hwnd);
        app
    }

    fn ensure_overlay_window(&mut self, frame: &eframe::Frame) {
        if self.overlay_hwnd == 0 {
            self.overlay_hwnd = extract_hwnd(frame);
        }
        self.ensure_overlay_window_by_hwnd(self.overlay_hwnd);
    }

    /// 首帧确保拿到 HWND 并应用非激活扩展样式 + 子类化 + 立即停靠到屏幕外（幂等）。
    fn ensure_overlay_window_by_hwnd(&mut self, hwnd: isize) {
        self.overlay_hwnd = hwnd;
        if hwnd == 0 || self.styles_applied {
            return;
        }
        self.styles_applied = window_ext::apply_overlay_ex_styles(hwnd);
        if self.styles_applied {
            window_ext::install_noactivate_subclass(hwnd);
            window_ext::park(hwnd);
            log::debug!("[overlay] non-activating styles + subclass applied to hwnd={hwnd}");
        }
    }

    /// 执行控制器返回的一批副作用。
    fn apply_effects(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Dock {
                    x,
                    y,
                    width,
                    height,
                } => window_ext::dock(self.overlay_hwnd, x, y, width, height),
                Effect::Park => window_ext::park(self.overlay_hwnd),
                Effect::Inject { hwnd, path } => crate::os::dialog::inject_folder_path(hwnd, &path),
                Effect::SetHookActive(active) => input_hook::set_active(active),
                Effect::RefreshPaths => self.controller.set_paths(explorer::get_open_windows()),
            }
        }
    }
}

impl eframe::App for PathWarpApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // 透明背景，让悬浮卡片的圆角与阴影落在透明区域上。
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.ensure_overlay_window(frame);

        let env = Env {
            now: Instant::now(),
            foreground_hwnd: monitor::foreground_hwnd(),
        };

        // 1. 排空对话框状态通道。
        let mut effects = Vec::new();
        for msg in self.dialog_rx.try_iter() {
            effects.extend(self.controller.step(env, Event::DialogUpdate(msg)));
        }
        // 2. 排空键盘钩子通道。
        for key in self.key_rx.try_iter() {
            effects.extend(self.controller.step(env, Event::Key(key)));
        }
        // 3. 心跳，推进去抖/宽限计时并做显隐收敛。
        effects.extend(self.controller.step(env, Event::Tick));
        self.apply_effects(effects);

        // 4. 可见时渲染，并把鼠标交互回喂控制器。
        if self.controller.is_visible()
            && let Some(ui_event) = crate::ui::window::render(root, &self.controller)
        {
            let event = match ui_event {
                UiEvent::ItemClicked(idx) => Event::ItemClicked(idx),
                UiEvent::ItemDoubleClicked(idx) => Event::ItemDoubleClicked(idx),
            };
            let fx = self.controller.step(env, event);
            self.apply_effects(fx);
        }

        // 会话进行期间持续心跳；空闲时停止重绘以省电（新对话框由 monitor 唤醒）。
        if self.controller.needs_tick() {
            root.ctx()
                .request_repaint_after(Duration::from_millis(UI_TICK_MS));
        }
    }
}
