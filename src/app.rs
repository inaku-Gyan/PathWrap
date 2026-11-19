//! eframe 应用外壳：把外部事件（对话框通道、egui 输入和鼠标响应）喂给纯
//! 控制器 [`crate::core::controller::Controller`]，并执行控制器返回的 [`Effect`]。
//! 本文件不含任何显隐/停靠/注入/去抖判断——那些全在控制器里，可被单测覆盖。

use crate::config::{AppConfig, ConfigStore, ThemeMode, ThemePreference};
use crate::core::controller::{Controller, Effect, Env, Event};
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
    /// 启动阶段是否已 park 过一次（避免默认位置残留）。
    parked_once: bool,

    dialog_rx: Receiver<Option<DialogInfo>>,

    theme_preference: ThemePreference,
    theme_mode: ThemeMode,
    config_store: ConfigStore,
    controller: Controller,
    navigator: crate::os::dialog::Navigator,
    shortcut: Option<crate::os::shortcut::Listener>,
    desktop: Option<crate::os::desktop::Desktop>,
    registration: pathwarp::startup::Registration<crate::os::startup::RunRegistration>,
    settings_error: Option<String>,
}

impl PathWarpApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        dialog_rx: Receiver<Option<DialogInfo>>,
        theme_preference: ThemePreference,
        config_store: ConfigStore,
    ) -> Self {
        let shortcut = crate::os::shortcut::Listener::start()
            .map_err(|error| log::warn!("Ctrl+G is unavailable: {error}"))
            .ok();
        let mut controller = Controller::new();
        controller.set_shortcut_available(shortcut.is_some());
        let registration = pathwarp::startup::Registration::new(
            crate::os::startup::RunRegistration::default(),
            startup_target(&config_store),
        );
        let view = pathwarp::settings::View::new(theme_preference, &config_store, &registration);
        let (desktop, settings_error) =
            match crate::os::desktop::Desktop::start(cc.egui_ctx.clone(), extract_hwnd(cc), view) {
                Ok(desktop) => (Some(desktop), None),
                Err(error) => {
                    log::error!("settings/tray unavailable: {error}");
                    (None, Some(format!("设置入口不可用：{error}")))
                }
            };
        let mut app = Self {
            overlay_hwnd: extract_hwnd(cc),
            parked_once: false,
            dialog_rx,
            theme_mode: match theme_preference {
                ThemePreference::Auto => crate::ui::theme::system_theme(),
                ThemePreference::Dark => ThemeMode::Dark,
                ThemePreference::Light => ThemeMode::Light,
            },
            theme_preference,
            config_store,
            controller,
            navigator: crate::os::dialog::Navigator::default(),
            shortcut,
            desktop,
            registration,
            settings_error,
        };
        // 尽早应用窗口样式并停靠到屏幕外，避免启动时窗口在默认位置可见。
        app.ensure_overlay_window(None);
        app
    }

    /// 每帧确保悬浮窗的扩展样式就绪（幂等）。
    ///
    /// 关键：扩展样式**每帧重新断言**——winit 在启动/显示阶段会用自己算出的
    /// `GWL_EXSTYLE` 覆盖我们首次设置的值，单次设置守不住。`apply_overlay_ex_styles`
    /// 幂等：每帧重新断言工具窗口、置顶和可激活状态。首次 park 只做一次。
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

        window_ext::apply_overlay_ex_styles(hwnd);
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
        self.config_store.save(&config);
    }

    fn toggle_theme_preference(&mut self) -> ThemePreference {
        match self.theme_mode {
            ThemeMode::Light => ThemePreference::Dark,
            ThemeMode::Dark => ThemePreference::Light,
        }
    }

    fn process_settings(&mut self, ctx: &egui::Context) {
        use crate::os::desktop::Command;
        let commands = self
            .desktop
            .as_ref()
            .map(|desktop| desktop.drain())
            .unwrap_or_default();
        for command in commands {
            match command {
                Command::Interaction(active) => {
                    let effects = self.controller.step(
                        Env {
                            now: Instant::now(),
                            foreground_hwnd: monitor::foreground_hwnd(),
                        },
                        Event::SettingsInteraction(active),
                    );
                    if active {
                        self.navigator.fence(None, monitor::foreground_hwnd());
                    }
                    self.apply_effects(effects);
                    if let Some(shortcut) = &self.shortcut {
                        shortcut.set_scope(self.controller.focus_request(), self.overlay_hwnd);
                    }
                }
                Command::Opened | Command::Refresh => {
                    self.registration.target = startup_target(&self.config_store);
                    self.registration.refresh();
                }
                Command::Theme(preference) => self.set_theme_preference(ctx, preference),
                Command::RetrySave => {
                    self.config_store.retry_save(&AppConfig {
                        theme: self.theme_preference,
                    });
                }
                Command::Startup(action, revision) => {
                    self.registration.target = startup_target(&self.config_store);
                    self.registration.execute(action, revision);
                }
                Command::Diagnostic(error) => {
                    if let Some(error) = &error {
                        log::warn!("{error}");
                    }
                    self.settings_error = error;
                }
            }
        }
        if let Some(desktop) = &self.desktop {
            let mut view = pathwarp::settings::View::new(
                self.theme_preference,
                &self.config_store,
                &self.registration,
            );
            if let Some(error) = &self.settings_error {
                view.theme_status.push_str(&format!("\r\n{error}"));
            }
            desktop.update(view);
        }
    }

    fn open_settings(&mut self, ctx: &egui::Context) {
        if self.desktop.is_none() {
            let view = pathwarp::settings::View::new(
                self.theme_preference,
                &self.config_store,
                &self.registration,
            );
            match crate::os::desktop::Desktop::start(ctx.clone(), self.overlay_hwnd, view) {
                Ok(desktop) => {
                    self.desktop = Some(desktop);
                    self.settings_error = None;
                }
                Err(error) => {
                    self.settings_error = Some(format!("设置入口不可用：{error}"));
                    return;
                }
            }
        }
        let effects = self.controller.step(
            Env {
                now: Instant::now(),
                foreground_hwnd: monitor::foreground_hwnd(),
            },
            Event::SettingsInteraction(true),
        );
        self.navigator.fence(None, monitor::foreground_hwnd());
        self.apply_effects(effects);
        if let Some(desktop) = &self.desktop {
            desktop.open();
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
                Effect::Navigate(mut request) => {
                    request.overlay_hwnd = self.overlay_hwnd;
                    if !self.navigator.start(request.clone()) {
                        let mut report = crate::core::navigation::Report::new(request);
                        report.reason = "navigation supervisor is busy".into();
                        let effects = self.controller.step(
                            Env {
                                now: Instant::now(),
                                foreground_hwnd: monitor::foreground_hwnd(),
                            },
                            Event::NavigationFinished(report),
                        );
                        self.apply_effects(effects);
                    }
                    if let Some(desktop) = &self.desktop {
                        desktop.set_cancellation(self.navigator.cancellation());
                    }
                }
                Effect::Activate {
                    session,
                    dialog,
                    overlay,
                } => {
                    let foreground = monitor::foreground_hwnd();
                    if self.controller.active_session() == Some(session)
                        && crate::os::dialog::identity(dialog.hwnd) == Some(dialog)
                        && (foreground == dialog.hwnd || foreground == self.overlay_hwnd)
                    {
                        let target = if overlay {
                            self.overlay_hwnd
                        } else {
                            dialog.hwnd
                        };
                        self.controller
                            .activation_finished(window_ext::activate(target));
                    }
                }
                Effect::RefreshPaths => self.controller.set_paths(explorer::get_open_windows()),
            }
        }
    }
}

impl eframe::App for PathWarpApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_settings(ctx);
    }
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // 透明背景，让悬浮卡片的圆角与阴影落在透明区域上。
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.ensure_overlay_window(Some(frame));
        self.process_settings(root.ctx());
        self.refresh_theme(root.ctx());

        let foreground_hwnd = monitor::foreground_hwnd();
        self.controller
            .set_overlay_focused(self.overlay_hwnd != 0 && foreground_hwnd == self.overlay_hwnd);
        let env = Env {
            now: Instant::now(),
            foreground_hwnd,
        };

        // 1. 排空对话框状态通道。
        let mut effects = Vec::new();
        for msg in self.dialog_rx.try_iter() {
            effects.extend(self.controller.step(env, Event::DialogUpdate(msg)));
        }
        // 2. 心跳，推进去抖/宽限计时并做显隐收敛。
        effects.extend(self.controller.step(env, Event::Tick));
        self.navigator
            .fence(self.controller.active_session(), monitor::foreground_hwnd());
        if let Some(report) = self.navigator.poll() {
            if !report.outcome.is_success() {
                log::warn!(
                    "navigation {:?} at {:?}: {}",
                    report.outcome,
                    report.stage,
                    report.reason
                );
            }
            effects.extend(self.controller.step(env, Event::NavigationFinished(report)));
        }
        if let Some(shortcut) = &self.shortcut {
            shortcut.set_scope(self.controller.focus_request(), self.overlay_hwnd);
            if let Some(available) = shortcut.take_availability() {
                self.controller.set_shortcut_available(available);
            }
            if let Some(result) = shortcut.take_result() {
                let foreground_hwnd = monitor::foreground_hwnd();
                self.controller
                    .set_overlay_focused(foreground_hwnd == self.overlay_hwnd);
                effects.extend(self.controller.step(
                    Env {
                        now: Instant::now(),
                        foreground_hwnd,
                    },
                    Event::FocusFinished(result),
                ));
            }
        }
        self.apply_effects(effects);

        // 4. 可见时渲染，并把鼠标交互回喂控制器。
        if self.controller.is_visible()
            && let Some(ui_event) = crate::ui::window::render_with_status(
                root,
                &mut self.controller,
                self.theme_preference,
                self.theme_mode,
                if matches!(
                    self.config_store.save_state(),
                    crate::config::SaveState::Unsaved(_)
                ) {
                    Some("主题未保存，仅本次生效。可从设置重试保存。")
                } else {
                    self.settings_error.as_deref()
                },
            )
        {
            let event = match ui_event {
                UiEvent::Search => None,
                UiEvent::Key(key) => Some(Event::Key(key)),
                UiEvent::Item(idx) => Some(Event::ItemClicked(idx)),
                UiEvent::ItemDouble(idx) => Some(Event::ItemDoubleClicked(idx)),
                UiEvent::Theme(action) => {
                    let preference = match action {
                        ThemeAction::Toggle => self.toggle_theme_preference(),
                        ThemeAction::Set(preference) => preference,
                    };
                    self.set_theme_preference(root.ctx(), preference);
                    None
                }
                UiEvent::Settings => {
                    self.open_settings(root.ctx());
                    None
                }
            };
            if let Some(event) = event {
                let fx = self.controller.step(env, event);
                // Escape invalidates the interaction before parking. Notify the
                // worker now, even when the native foreground is unchanged.
                self.navigator
                    .fence(self.controller.active_session(), monitor::foreground_hwnd());
                self.apply_effects(fx);
            }
        }

        // Escape or a UI action may have ended the session during this frame.
        if let Some(shortcut) = &self.shortcut {
            shortcut.set_scope(self.controller.focus_request(), self.overlay_hwnd);
        }

        // 会话进行期间持续心跳；空闲时停止重绘以省电（新对话框由 monitor 唤醒）。
        if self.controller.needs_tick() {
            root.ctx()
                .request_repaint_after(Duration::from_millis(UI_TICK_MS));
        }
    }
}

fn startup_target(store: &ConfigStore) -> Result<pathwarp::startup::Target, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let storage = store.storage_directory().ok_or("无法确认配置目录。")?;
    pathwarp::startup::Target::current(&executable, storage)
}
