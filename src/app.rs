//! eframe 应用外壳：把外部事件（对话框通道、键盘钩子、egui 鼠标响应）喂给纯
//! 控制器 [`crate::core::controller::Controller`]，并执行控制器返回的 [`Effect`]。
//! 本文件不含任何显隐/停靠/注入/去抖判断——那些全在控制器里，可被单测覆盖。

use crate::config::{AppConfig, ThemeMode, ThemePreference};
use crate::core::controller::{Controller, Effect, Env, Event};
use crate::os::input_hook::{self, KeyAction};
use crate::os::monitor::{self, DialogInfo};
use crate::os::{explorer, window_ext};
use crate::ui::window::{ThemeAction, UiEvent};
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
    /// 子类化只装一次（幂等，装成功后置真）。
    subclassed: bool,
    /// 启动阶段是否已 park 过一次（避免默认位置残留）。
    parked_once: bool,

    dialog_rx: Receiver<Option<DialogInfo>>,
    /// 低层键盘钩子送来的输入意图（悬浮条为非激活窗，无法用 egui 接收键盘）。
    key_rx: Receiver<KeyAction>,

    theme_preference: ThemePreference,
    theme_mode: ThemeMode,
    controller: Controller,
}

impl PathWarpApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        dialog_rx: Receiver<Option<DialogInfo>>,
        key_rx: Receiver<KeyAction>,
        theme_preference: ThemePreference,
    ) -> Self {
        let mut app = Self {
            overlay_hwnd: extract_hwnd(cc),
            subclassed: false,
            parked_once: false,
            dialog_rx,
            key_rx,
            theme_mode: match theme_preference {
                ThemePreference::Auto => crate::ui::theme::system_theme(),
                ThemePreference::Dark => ThemeMode::Dark,
                ThemePreference::Light => ThemeMode::Light,
            },
            theme_preference,
            controller: Controller::new(),
        };
        // 尽早应用非激活样式并停靠到屏幕外，避免启动时窗口在默认位置可见。
        app.ensure_overlay_window(None);
        app
    }

    /// 每帧确保悬浮窗的非激活状态就绪（幂等）。
    ///
    /// 关键：扩展样式**每帧重新断言**——winit 在启动/显示阶段会用自己算出的
    /// `GWL_EXSTYLE` 覆盖我们首次设置的值（实测会抹掉 `WS_EX_NOACTIVATE`/`TOOLWINDOW`），
    /// 单次设置守不住。`apply_overlay_ex_styles` 幂等：位齐了就只读不写。子类化与
    /// 首次 park 各只做一次。
    fn ensure_overlay_window(&mut self, frame: Option<&eframe::Frame>) {
        if self.overlay_hwnd == 0
            && let Some(frame) = frame
        {
            self.overlay_hwnd = extract_hwnd(frame);
        }
        let hwnd = self.overlay_hwnd;
        if hwnd == 0 {
            return;
        }

        input_hook::set_overlay_hwnd(hwnd);

        window_ext::apply_overlay_ex_styles(hwnd);

        if !self.subclassed {
            self.subclassed = window_ext::install_noactivate_subclass(hwnd);
            log::debug!(
                "[overlay] hwnd={hwnd} subclass_installed={}",
                self.subclassed
            );
        }
        if !self.parked_once {
            window_ext::park(hwnd);
            self.parked_once = true;
        }
    }

    /// Re-resolve `auto` when an egui repaint is already needed.  This keeps a
    /// visible overlay in sync with Windows theme changes without adding an
    /// always-on polling timer or touching window activation behavior.
    fn refresh_theme(&mut self, ctx: &egui::Context) {
        let ThemePreference::Auto = self.theme_preference else {
            return;
        };
        let mode = crate::ui::theme::system_theme();
        if mode != self.theme_mode {
            crate::ui::theme::apply_theme(ctx, mode);
            self.theme_mode = mode;
        }
    }

    /// Apply and persist a theme choice made from the compact header control.
    fn set_theme_preference(&mut self, ctx: &egui::Context, preference: ThemePreference) {
        let mode = preference.resolve(crate::ui::theme::system_theme());
        crate::ui::theme::apply_theme(ctx, mode);
        self.theme_preference = preference;
        self.theme_mode = mode;

        let config = AppConfig { theme: preference };
        if let Err(error) = config.save() {
            log::warn!("could not persist theme preference: {error}");
        }
    }

    fn toggle_theme_preference(&mut self) -> ThemePreference {
        match self.theme_mode {
            ThemeMode::Light => ThemePreference::Dark,
            ThemeMode::Dark => ThemePreference::Light,
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
        self.ensure_overlay_window(Some(frame));
        self.refresh_theme(root.ctx());

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
            && let Some(ui_event) = crate::ui::window::render_with_theme_and_capture(
                root,
                &self.controller,
                self.theme_preference,
                self.theme_mode,
                input_hook::capture_active(),
            )
        {
            let event = match ui_event {
                UiEvent::Search => {
                    input_hook::set_capture_active(true);
                    root.ctx().request_repaint();
                    None
                }
                UiEvent::Item(idx) => {
                    input_hook::set_capture_active(true);
                    root.ctx().request_repaint();
                    Some(Event::ItemClicked(idx))
                }
                UiEvent::ItemDouble(idx) => {
                    input_hook::set_capture_active(true);
                    root.ctx().request_repaint();
                    Some(Event::ItemDoubleClicked(idx))
                }
                UiEvent::Theme(action) => {
                    let preference = match action {
                        ThemeAction::Toggle => self.toggle_theme_preference(),
                        ThemeAction::Set(preference) => preference,
                    };
                    self.set_theme_preference(root.ctx(), preference);
                    None
                }
            };
            if let Some(event) = event {
                let fx = self.controller.step(env, event);
                self.apply_effects(fx);
            }
        }

        // 会话进行期间持续心跳；空闲时停止重绘以省电（新对话框由 monitor 唤醒）。
        if self.controller.needs_tick() {
            root.ctx()
                .request_repaint_after(Duration::from_millis(UI_TICK_MS));
        }
    }
}
