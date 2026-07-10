//! 悬浮条的纯控制器状态机——平台无关，零 Win32 依赖，全部决策集中于此。
//!
//! 设计：`step(env, event) -> Vec<Effect>`。时间由 [`Env::now`] 注入，
//! 前台窗口由 [`Env::foreground_hwnd`] 注入，因此去抖/宽限等时序逻辑可确定性
//! 单测。[`crate::app`] 只负责把外部事件喂进来、并执行返回的 [`Effect`]，
//! 自身不做任何显隐/停靠/注入判断。

use crate::core::types::{DialogInfo, KeyAction};
use std::time::{Duration, Instant};

/// 悬浮条逻辑高度（像素，按对话框 DPI 缩放为物理像素）。
const OVERLAY_HEIGHT_LOGICAL: u32 = 140;
/// 悬浮条与对话框下边缘的物理像素间距（0 = 紧贴）。
const OVERLAY_GAP: i32 = 0;
/// `DialogUpdate(None)` 到达后延迟隐藏的去抖窗口，吸收打开/保存态切换的瞬时丢失。
const HIDE_GRACE: Duration = Duration::from_millis(120);
/// 目标对话框失去前台后延迟隐藏的宽限，吸收前台瞬时抖动（含点击悬浮条的边缘情形）。
const FG_LOST_GRACE: Duration = Duration::from_millis(150);

/// 每次 `step` 注入的外部环境快照。
#[derive(Debug, Clone, Copy)]
pub struct Env {
    pub now: Instant,
    pub foreground_hwnd: isize,
}

/// 喂给控制器的一次外部事件。
#[derive(Debug, Clone)]
pub enum Event {
    /// 监视线程上报的对话框状态（`Some` = 出现/更新，`None` = 疑似消失）。
    DialogUpdate(Option<DialogInfo>),
    /// 键盘钩子送来的输入意图。
    Key(KeyAction),
    /// 列表项被单击（下标为过滤后列表中的位置）。
    ItemClicked(usize),
    /// 列表项被双击。
    ItemDoubleClicked(usize),
    /// 无外部事件的心跳帧，用于推进去抖/宽限计时。
    Tick,
}

/// 控制器要求宿主执行的一个副作用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// 以物理像素把悬浮条停靠到该矩形并显示（不抢焦点）。
    Dock {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    },
    /// 隐藏悬浮条（移到屏幕外）。
    Park,
    /// 把 `path` 注入 `hwnd` 指向的对话框并确认。
    Inject { hwnd: isize, path: String },
    /// 设置键盘钩子门控（是否截获打字/导航键）。
    SetHookActive(bool),
    /// 重新读取已打开的 Explorer 路径列表。
    RefreshPaths,
}

#[derive(Default)]
pub struct Controller {
    // ---- 会话/显隐状态 ----
    target_dialog: Option<DialogInfo>,
    session_hwnd: Option<isize>,
    pending_none_since: Option<Instant>,
    fg_lost_since: Option<Instant>,
    /// 用户按 ESC / 完成注入后被抑制的对话框，避免同一会话立即被重新拉起。
    user_hidden_hwnd: Option<isize>,

    // ---- 已下发效果的去重基线 ----
    visible: bool,
    hook_active: bool,
    last_dock: Option<(i32, i32, i32, i32)>,
    /// 新会话请求刷新路径，在下一次 reconcile 里发一次 `RefreshPaths`。
    pending_refresh: bool,

    // ---- 交互模型 ----
    paths: Vec<String>,
    query: String,
    selected: usize,
}

impl Controller {
    pub fn new() -> Self {
        Self::default()
    }

    // ---- 供渲染层读取的模型快照 ----

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 会话进行中（有目标对话框或正在去抖），宿主应继续心跳重绘以推进计时。
    pub fn needs_tick(&self) -> bool {
        self.target_dialog.is_some() || self.pending_none_since.is_some()
    }

    /// 大小写不敏感地按查询过滤后的路径列表。
    pub fn filtered_paths(&self) -> Vec<String> {
        let q = self.query.to_lowercase();
        self.paths
            .iter()
            .filter(|p| p.to_lowercase().contains(&q))
            .cloned()
            .collect()
    }

    /// 由 `RefreshPaths` 效果驱动：写入最新的路径列表并夹取选中项。
    pub fn set_paths(&mut self, paths: Vec<String>) {
        self.paths = paths;
        self.clamp_selection();
    }

    // ---- 状态机入口 ----

    pub fn step(&mut self, env: Env, event: Event) -> Vec<Effect> {
        let mut fx = Vec::new();
        match event {
            Event::DialogUpdate(update) => self.on_dialog_update(update, &env),
            Event::Key(key) => self.on_key(key, &mut fx),
            Event::ItemClicked(index) => self.select_filtered(index),
            Event::ItemDoubleClicked(index) => {
                self.select_filtered(index);
                self.confirm(&mut fx);
            }
            Event::Tick => {}
        }
        self.reconcile(&env, &mut fx);
        fx
    }

    fn on_dialog_update(&mut self, update: Option<DialogInfo>, env: &Env) {
        match update {
            Some(info) => {
                // 抑制中的同一对话框：不重新拉起。
                if self.user_hidden_hwnd == Some(info.hwnd) {
                    self.target_dialog = None;
                    self.pending_none_since = None;
                    return;
                }
                // 切到了不同对话框：解除上一次的抑制。
                if self.user_hidden_hwnd.is_some() {
                    self.user_hidden_hwnd = None;
                }

                self.target_dialog = Some(info);
                self.pending_none_since = None;

                // 新会话（对话框句柄变化）：刷新路径并重置筛选。
                if self.session_hwnd != Some(info.hwnd) {
                    self.session_hwnd = Some(info.hwnd);
                    self.query.clear();
                    self.selected = 0;
                    self.pending_refresh = true;
                }
            }
            None => {
                if self.pending_none_since.is_none() {
                    self.pending_none_since = Some(env.now);
                }
            }
        }
    }

    fn on_key(&mut self, key: KeyAction, fx: &mut Vec<Effect>) {
        match key {
            KeyAction::Char(c) => {
                self.query.push(c);
                self.clamp_selection();
            }
            KeyAction::Backspace => {
                self.query.pop();
                self.clamp_selection();
            }
            KeyAction::Up => self.move_selection(true, false),
            KeyAction::Down => self.move_selection(false, true),
            KeyAction::Enter => self.confirm(fx),
            KeyAction::Escape => self.suppress_current_session(),
        }
    }

    /// 完成选择：先关钩子门控，再注入选中路径——**注入后保持停靠**，让用户可继续
    /// 挑选/输入。收起悬浮条是 ESC 的职责，不在此处。
    ///
    /// 关钩子只是为了 UIA 同步调用期间不吞键：注入完成后，同一 `step` 的 `reconcile`
    /// 见「目标仍在 + 对话框仍前台」会自动重新 `SetHookActive(true)`，故净效果为
    /// `[SetHookActive(false), Inject, SetHookActive(true)]`，钩子随即恢复。
    fn confirm(&mut self, fx: &mut Vec<Effect>) {
        let Some(dialog) = self.target_dialog else {
            return;
        };
        let Some(path) = self.selected_path() else {
            return;
        };
        // 注入前必须先关闭钩子门控，避免 UIA 同步调用期间仍在吞键。
        if self.hook_active {
            fx.push(Effect::SetHookActive(false));
            self.hook_active = false;
        }
        fx.push(Effect::Inject {
            hwnd: dialog.hwnd,
            path,
        });
    }

    /// ESC：抑制当前对话框会话，收起悬浮条。
    fn suppress_current_session(&mut self) {
        self.user_hidden_hwnd = self.target_dialog.map(|d| d.hwnd);
        self.target_dialog = None;
        self.session_hwnd = None;
        self.pending_none_since = None;
        self.fg_lost_since = None;
        self.query.clear();
        self.selected = 0;
    }

    /// 会话彻底结束（对话框消失并超过去抖）：清空一切并解除抑制。
    fn end_session(&mut self) {
        self.target_dialog = None;
        self.session_hwnd = None;
        self.pending_none_since = None;
        self.fg_lost_since = None;
        self.user_hidden_hwnd = None;
        self.query.clear();
        self.selected = 0;
    }

    fn reconcile(&mut self, env: &Env, fx: &mut Vec<Effect>) {
        // 惰性刷新：新会话请求过一次路径刷新。
        if self.pending_refresh {
            self.pending_refresh = false;
            fx.push(Effect::RefreshPaths);
        }

        // 推进 `DialogUpdate(None)` 去抖计时。
        if let Some(since) = self.pending_none_since
            && env.now.duration_since(since) >= HIDE_GRACE
        {
            self.end_session();
        }

        // 计算期望显隐：目标存在且（前台命中，或前台丢失仍在宽限内）。
        let desired_visible = match self.target_dialog {
            None => false,
            Some(dialog) => {
                if env.foreground_hwnd == dialog.hwnd {
                    self.fg_lost_since = None;
                    true
                } else {
                    let since = *self.fg_lost_since.get_or_insert(env.now);
                    env.now.duration_since(since) < FG_LOST_GRACE
                }
            }
        };

        if desired_visible {
            if let Some(dialog) = self.target_dialog {
                let geom = dock_geometry(&dialog);
                if !self.visible || self.last_dock != Some(geom) {
                    fx.push(Effect::Dock {
                        x: geom.0,
                        y: geom.1,
                        width: geom.2,
                        height: geom.3,
                    });
                    self.last_dock = Some(geom);
                }
                if !self.hook_active {
                    fx.push(Effect::SetHookActive(true));
                    self.hook_active = true;
                }
                self.visible = true;
            }
        } else if self.visible {
            if self.hook_active {
                fx.push(Effect::SetHookActive(false));
                self.hook_active = false;
            }
            fx.push(Effect::Park);
            self.visible = false;
            self.last_dock = None;
        }
    }

    // ---- 选择/筛选辅助 ----

    fn selected_path(&self) -> Option<String> {
        self.filtered_paths().get(self.selected).cloned()
    }

    fn filtered_len(&self) -> usize {
        let q = self.query.to_lowercase();
        self.paths
            .iter()
            .filter(|p| p.to_lowercase().contains(&q))
            .count()
    }

    fn clamp_selection(&mut self) {
        let len = self.filtered_len();
        self.selected = if len == 0 {
            0
        } else {
            self.selected.min(len - 1)
        };
    }

    fn move_selection(&mut self, up: bool, down: bool) {
        let len = self.filtered_len();
        self.clamp_selection();
        if up {
            self.selected = self.selected.saturating_sub(1);
        }
        if down && len > 0 {
            self.selected = (self.selected + 1).min(len - 1);
        }
    }

    fn select_filtered(&mut self, index: usize) {
        let len = self.filtered_len();
        if len == 0 {
            self.selected = 0;
        } else {
            self.selected = index.min(len - 1);
        }
    }
}

/// 计算悬浮条停靠矩形：对话框正下方，宽度对齐，高度按 DPI 缩放。
fn dock_geometry(dialog: &DialogInfo) -> (i32, i32, i32, i32) {
    let scaled = OVERLAY_HEIGHT_LOGICAL.saturating_mul(dialog.dpi) / 96;
    let height = i32::try_from(scaled).unwrap_or(OVERLAY_HEIGHT_LOGICAL as i32);
    (
        dialog.x,
        dialog.y + dialog.height + OVERLAY_GAP,
        dialog.width,
        height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog(hwnd: isize) -> DialogInfo {
        DialogInfo {
            hwnd,
            x: 100,
            y: 200,
            width: 600,
            height: 400,
            dpi: 96,
        }
    }

    /// 便于断言的效果查询辅助。
    fn has_dock(fx: &[Effect]) -> bool {
        fx.iter().any(|e| matches!(e, Effect::Dock { .. }))
    }
    fn has_park(fx: &[Effect]) -> bool {
        fx.iter().any(|e| matches!(e, Effect::Park))
    }
    fn hook_set(fx: &[Effect]) -> Option<bool> {
        fx.iter().rev().find_map(|e| match e {
            Effect::SetHookActive(v) => Some(*v),
            _ => None,
        })
    }
    fn inject_path(fx: &[Effect]) -> Option<String> {
        fx.iter().find_map(|e| match e {
            Effect::Inject { path, .. } => Some(path.clone()),
            _ => None,
        })
    }

    fn base() -> Instant {
        Instant::now()
    }

    /// 让控制器进入“对话框已停靠”的稳定态，返回起始时刻。
    fn dock_at(c: &mut Controller, hwnd: isize) -> Instant {
        let t = base();
        let fx = c.step(
            Env {
                now: t,
                foreground_hwnd: hwnd,
            },
            Event::DialogUpdate(Some(dialog(hwnd))),
        );
        assert!(has_dock(&fx), "expected initial dock");
        t
    }

    #[test]
    fn dialog_appears_docks_activates_hook_refreshes() {
        let mut c = Controller::new();
        let t = base();
        let fx = c.step(
            Env {
                now: t,
                foreground_hwnd: 1,
            },
            Event::DialogUpdate(Some(dialog(1))),
        );
        assert!(has_dock(&fx));
        assert_eq!(hook_set(&fx), Some(true));
        assert!(fx.contains(&Effect::RefreshPaths));
        // 停靠几何：对话框正下方，宽度对齐。
        assert!(fx.contains(&Effect::Dock {
            x: 100,
            y: 600,
            width: 600,
            height: 140,
        }));
    }

    #[test]
    fn transient_foreground_loss_under_grace_does_not_park() {
        let mut c = Controller::new();
        let t = dock_at(&mut c, 1);

        // 50ms 前台丢失（<150ms 宽限）：不得 Park。
        let fx = c.step(
            Env {
                now: t + Duration::from_millis(50),
                foreground_hwnd: 999,
            },
            Event::Tick,
        );
        assert!(!has_park(&fx), "must not park within foreground grace");

        // 100ms 时前台回归：仍不 Park（这正是点击悬浮条边缘情形的逻辑级回归）。
        let fx = c.step(
            Env {
                now: t + Duration::from_millis(100),
                foreground_hwnd: 1,
            },
            Event::Tick,
        );
        assert!(!has_park(&fx));
        assert!(c.is_visible());
    }

    #[test]
    fn sustained_foreground_loss_parks_and_deactivates_hook() {
        let mut c = Controller::new();
        let t = dock_at(&mut c, 1);

        // 第一帧观察到前台丢失：启动宽限计时，尚不 Park。
        let fx = c.step(
            Env {
                now: t + Duration::from_millis(10),
                foreground_hwnd: 999,
            },
            Event::Tick,
        );
        assert!(!has_park(&fx));

        // 持续丢失超过 150ms：Park + 关钩子。
        let fx = c.step(
            Env {
                now: t + Duration::from_millis(200),
                foreground_hwnd: 999,
            },
            Event::Tick,
        );
        assert!(has_park(&fx));
        assert_eq!(hook_set(&fx), Some(false));
        assert!(!c.is_visible());
    }

    #[test]
    fn foreground_returns_after_park_redocks() {
        let mut c = Controller::new();
        let t = dock_at(&mut c, 1);
        // 先持续丢失前台 → Park（两帧：起表 + 超时）。
        c.step(
            Env {
                now: t + Duration::from_millis(10),
                foreground_hwnd: 999,
            },
            Event::Tick,
        );
        c.step(
            Env {
                now: t + Duration::from_millis(200),
                foreground_hwnd: 999,
            },
            Event::Tick,
        );
        assert!(!c.is_visible());
        // 前台回到对话框（对话框仍在跟踪）→ 重新 Dock 自愈。
        let fx = c.step(
            Env {
                now: t + Duration::from_millis(300),
                foreground_hwnd: 1,
            },
            Event::Tick,
        );
        assert!(has_dock(&fx));
        assert_eq!(hook_set(&fx), Some(true));
    }

    #[test]
    fn escape_parks_and_suppresses_same_dialog() {
        let mut c = Controller::new();
        let t = dock_at(&mut c, 1);

        let fx = c.step(
            Env {
                now: t + Duration::from_millis(10),
                foreground_hwnd: 1,
            },
            Event::Key(KeyAction::Escape),
        );
        assert!(has_park(&fx));
        assert_eq!(hook_set(&fx), Some(false));

        // 同一对话框再次上报：不得重新 Dock。
        let fx = c.step(
            Env {
                now: t + Duration::from_millis(20),
                foreground_hwnd: 1,
            },
            Event::DialogUpdate(Some(dialog(1))),
        );
        assert!(!has_dock(&fx), "suppressed dialog must not re-dock");
    }

    #[test]
    fn switching_dialog_releases_suppression() {
        let mut c = Controller::new();
        let t = dock_at(&mut c, 1);
        c.step(
            Env {
                now: t,
                foreground_hwnd: 1,
            },
            Event::Key(KeyAction::Escape),
        );
        // 换到不同对话框 → 解除抑制并 Dock。
        let fx = c.step(
            Env {
                now: t + Duration::from_millis(5),
                foreground_hwnd: 2,
            },
            Event::DialogUpdate(Some(dialog(2))),
        );
        assert!(has_dock(&fx));
    }

    #[test]
    fn dialog_none_after_grace_parks_and_clears() {
        let mut c = Controller::new();
        let t = dock_at(&mut c, 1);
        c.set_paths(vec!["C:\\Work".into()]);
        c.step(
            Env {
                now: t,
                foreground_hwnd: 1,
            },
            Event::Key(KeyAction::Char('w')),
        );

        // None 到达，起动去抖。
        c.step(
            Env {
                now: t + Duration::from_millis(10),
                foreground_hwnd: 1,
            },
            Event::DialogUpdate(None),
        );
        // 超过 120ms 去抖 → Park + 清空。
        let fx = c.step(
            Env {
                now: t + Duration::from_millis(150),
                foreground_hwnd: 1,
            },
            Event::Tick,
        );
        assert!(has_park(&fx));
        assert_eq!(c.query(), "");
        assert!(!c.is_visible());
    }

    #[test]
    fn close_then_reopen_redocks() {
        let mut c = Controller::new();
        let t = dock_at(&mut c, 1);
        c.step(
            Env {
                now: t,
                foreground_hwnd: 1,
            },
            Event::DialogUpdate(None),
        );
        c.step(
            Env {
                now: t + Duration::from_millis(150),
                foreground_hwnd: 1,
            },
            Event::Tick,
        );
        assert!(!c.is_visible());

        // 重开新对话框 → 必须重新 Dock。
        let fx = c.step(
            Env {
                now: t + Duration::from_millis(300),
                foreground_hwnd: 7,
            },
            Event::DialogUpdate(Some(dialog(7))),
        );
        assert!(has_dock(&fx));
        assert!(fx.contains(&Effect::RefreshPaths));
    }

    #[test]
    fn typing_and_backspace_update_query() {
        let mut c = Controller::new();
        let t = dock_at(&mut c, 1);
        c.set_paths(vec!["C:\\Work".into(), "D:\\Games".into()]);

        let env = Env {
            now: t,
            foreground_hwnd: 1,
        };
        c.step(env, Event::Key(KeyAction::Char('w')));
        assert_eq!(c.query(), "w");
        assert_eq!(c.filtered_paths(), vec!["C:\\Work".to_string()]);

        c.step(env, Event::Key(KeyAction::Backspace));
        assert_eq!(c.query(), "");
        assert_eq!(c.filtered_paths().len(), 2);
    }

    #[test]
    fn enter_injects_selected_path_and_stays_docked() {
        let mut c = Controller::new();
        let t = dock_at(&mut c, 1);
        c.set_paths(vec!["C:\\Work".into(), "D:\\Games".into()]);

        let env = Env {
            now: t,
            foreground_hwnd: 1,
        };
        c.step(env, Event::Key(KeyAction::Down)); // 选中第 2 项
        let fx = c.step(env, Event::Key(KeyAction::Enter));

        // 顺序：SetHookActive(false) 必须在 Inject 之前（UIA 同步调用期间不吞键）。
        let hook_idx = fx.iter().position(|e| e == &Effect::SetHookActive(false));
        let inject_idx = fx.iter().position(|e| matches!(e, Effect::Inject { .. }));
        assert!(hook_idx.is_some() && inject_idx.is_some());
        assert!(hook_idx < inject_idx, "hook must be disabled before inject");
        assert_eq!(inject_path(&fx).as_deref(), Some("D:\\Games"));

        // 注入后悬浮条保持停靠，且钩子门控在同帧内自动恢复。
        assert!(!has_park(&fx), "overlay must stay docked after injection");
        assert!(c.is_visible());
        assert_eq!(
            hook_set(&fx),
            Some(true),
            "hook must be re-enabled after inject"
        );
    }

    #[test]
    fn double_click_injects_single_click_does_not() {
        let mut c = Controller::new();
        let t = dock_at(&mut c, 1);
        c.set_paths(vec!["C:\\Work".into(), "D:\\Games".into()]);
        let env = Env {
            now: t,
            foreground_hwnd: 1,
        };

        // 单击仅改选中，不注入、不 Park。
        let fx = c.step(env, Event::ItemClicked(1));
        assert!(inject_path(&fx).is_none());
        assert!(!has_park(&fx));
        assert_eq!(c.selected_index(), 1);

        // 双击注入对应路径，且悬浮条保持停靠（不 Park）。
        let fx = c.step(env, Event::ItemDoubleClicked(0));
        assert_eq!(inject_path(&fx).as_deref(), Some("C:\\Work"));
        assert!(!has_park(&fx), "overlay must stay docked after injection");
        assert!(c.is_visible());
    }

    #[test]
    fn dialog_move_redocks_but_identical_update_is_deduped() {
        let mut c = Controller::new();
        let t = dock_at(&mut c, 1);

        // 同一几何再次上报 → 无 Dock（去重）。
        let fx = c.step(
            Env {
                now: t + Duration::from_millis(5),
                foreground_hwnd: 1,
            },
            Event::DialogUpdate(Some(dialog(1))),
        );
        assert!(!has_dock(&fx), "identical geometry must not re-dock");

        // 对话框移动 → 增量 Dock。
        let mut moved = dialog(1);
        moved.x = 150;
        let fx = c.step(
            Env {
                now: t + Duration::from_millis(10),
                foreground_hwnd: 1,
            },
            Event::DialogUpdate(Some(moved)),
        );
        assert!(fx.contains(&Effect::Dock {
            x: 150,
            y: 600,
            width: 600,
            height: 140,
        }));
    }

    /// 回归：注入选中路径后，悬浮条必须**保持停靠**（不得 Park），且会话未被抑制，
    /// 用户可继续挑选并再次注入——修复「选中/回车条目后 GUI 不应关闭却关闭了」。
    #[test]
    fn confirm_injects_but_keeps_overlay_docked() {
        let mut c = Controller::new();
        let t = dock_at(&mut c, 1);
        c.set_paths(vec!["C:\\Work".into(), "D:\\Games".into()]);
        let env = Env {
            now: t,
            foreground_hwnd: 1,
        };

        // 双击注入 → 停靠保持。
        let fx = c.step(env, Event::ItemDoubleClicked(1));
        assert_eq!(inject_path(&fx).as_deref(), Some("D:\\Games"));
        assert!(!has_park(&fx), "overlay must stay docked after injection");
        assert!(c.is_visible());

        // 注入后仍可再次注入（会话未被抑制 → 与 ESC 的收起行为区分开）。
        let fx = c.step(env, Event::Key(KeyAction::Enter));
        assert!(
            inject_path(&fx).is_some(),
            "session must remain active so another item can be injected"
        );
        assert!(!has_park(&fx));
        assert!(c.is_visible());
    }

    /// 多个文件对话框同时打开时，GUI 只跟随「活动中（前台）」的那一个：监视器只上报
    /// 前台 `#32770`（见 monitor::get_active_file_dialog），控制器据此把悬浮条迁移到
    /// 当前活动对话框，绝不为后台对话框显示第二个悬浮条。
    #[test]
    fn overlay_follows_active_dialog_when_multiple_open() {
        let mut c = Controller::new();
        let t = base();

        // 对话框 A（hwnd 1）在前台：停靠到 A 正下方。
        let fx = c.step(
            Env {
                now: t,
                foreground_hwnd: 1,
            },
            Event::DialogUpdate(Some(dialog(1))),
        );
        assert!(fx.contains(&Effect::Dock {
            x: 100,
            y: 600,
            width: 600,
            height: 140,
        }));

        // 用户切到第二个对话框 B（hwnd 2，位置不同），B 成为前台，监视器改报 B。
        let mut b = dialog(2);
        b.x = 900;
        b.y = 100;
        let fx = c.step(
            Env {
                now: t + Duration::from_millis(8),
                foreground_hwnd: 2,
            },
            Event::DialogUpdate(Some(b)),
        );

        // 悬浮条迁移到活动中的 B（新会话 → 刷新路径 + 停靠到 B 的几何），只此一个。
        assert!(fx.contains(&Effect::Dock {
            x: 900,
            y: 500,
            width: 600,
            height: 140,
        }));
        assert!(fx.contains(&Effect::RefreshPaths));
        assert!(c.is_visible());
    }
}
