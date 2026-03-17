use crate::os::monitor::DialogInfo;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

const HIDE_GRACE_MS: u64 = 120;
const UI_TICK_MS: u64 = 30;
const OVERLAY_HEIGHT: f32 = 140.0;
const OVERLAY_GAP: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverlayState {
    Visible,
    HiddenByUser,
    HiddenBySystem,
}

pub struct PathWarpApp {
    pub dialog_rx: Receiver<Option<DialogInfo>>,
    pub target_dialog: Option<DialogInfo>,
    pub pending_none_since: Option<Instant>,

    pub paths: Vec<String>,
    pub search_query: String,
    pub selected_index: usize,

    overlay_state: OverlayState,
    user_hidden_dialog_hwnd: Option<isize>,
    last_dialog_focused: bool,
    pub last_applied_visible: Option<bool>,
    pub last_applied_dialog: Option<DialogInfo>,
    pub last_applied_scale: Option<f32>,
}

impl PathWarpApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, dialog_rx: Receiver<Option<DialogInfo>>) -> Self {
        Self {
            dialog_rx,
            target_dialog: None,
            pending_none_since: None,

            paths: crate::os::explorer::get_open_windows(),
            search_query: String::new(),
            selected_index: 0,

            overlay_state: OverlayState::HiddenBySystem,
            user_hidden_dialog_hwnd: None,
            last_dialog_focused: false,
            last_applied_visible: None,
            last_applied_dialog: None,
            last_applied_scale: None,
        }
    }

    fn transition_overlay_state(&mut self, next: OverlayState, reason: &str) {
        if self.overlay_state != next {
            println!(
                "[overlay] state transition: {:?} -> {:?} ({})",
                self.overlay_state, next, reason
            );
            self.overlay_state = next;
        }
    }

    pub fn set_overlay_visible(&mut self, ctx: &eframe::egui::Context, visible: bool) {
        if visible {
            self.transition_overlay_state(OverlayState::Visible, "overlay requested visible");
        } else if self.overlay_state == OverlayState::Visible {
            self.transition_overlay_state(
                OverlayState::HiddenBySystem,
                "overlay requested hidden while visible",
            );
        }

        if self.last_applied_visible == Some(visible) {
            return;
        }

        self.last_applied_visible = Some(visible);

        if !visible {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                egui::WindowLevel::Normal,
            ));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(1.0, 1.0)));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                -10000.0, -10000.0,
            )));
            self.last_applied_dialog = None;
            self.last_applied_scale = None;
        }
    }

    pub fn hide_overlay_by_user(&mut self, ctx: &eframe::egui::Context) {
        self.user_hidden_dialog_hwnd = self.target_dialog.map(|d| d.hwnd);
        self.transition_overlay_state(
            OverlayState::HiddenByUser,
            "user pressed ESC, suppress current dialog session",
        );
        self.target_dialog = None;
        self.pending_none_since = None;
        self.last_dialog_focused = false;
        self.search_query.clear();
        self.selected_index = 0;
        self.set_overlay_visible(ctx, false);
    }

    fn hide_overlay_by_system(&mut self, ctx: &eframe::egui::Context) {
        self.transition_overlay_state(
            OverlayState::HiddenBySystem,
            "dialog lost from monitor detection",
        );
        self.target_dialog = None;
        self.pending_none_since = None;
        self.last_dialog_focused = false;
        self.set_overlay_visible(ctx, false);
    }

    fn place_overlay_for_dialog(&mut self, ctx: &eframe::egui::Context, dialog: DialogInfo) {
        let was_dialog_focused = self.last_dialog_focused;
        let dialog_focused = crate::os::monitor::is_foreground_hwnd(dialog.hwnd);
        let focus_returned = dialog_focused && !was_dialog_focused;
        let focus_lost = !dialog_focused && was_dialog_focused;
        self.last_dialog_focused = dialog_focused;

        if focus_returned {
            println!(
                "[overlay] focus returned to dialog {}, apply one-shot topmost and resync",
                dialog.hwnd
            );
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                egui::WindowLevel::AlwaysOnTop,
            ));
            self.last_applied_dialog = None;
        } else if focus_lost {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                egui::WindowLevel::Normal,
            ));
        }

        let pixels_per_point = ctx.pixels_per_point();
        let needs_update = self.last_applied_dialog != Some(dialog)
            || self
                .last_applied_scale
                .map(|s| (s - pixels_per_point).abs() > 0.01)
                .unwrap_or(true)
            || self.last_applied_visible != Some(true)
            || focus_returned;

        if !needs_update {
            return;
        }

        let ui_height = OVERLAY_HEIGHT;
        let pos_x = dialog.x as f32 / pixels_per_point;
        let pos_y = (dialog.y + dialog.height) as f32 / pixels_per_point + OVERLAY_GAP;
        let width = dialog.width as f32 / pixels_per_point;

        let new_pos = egui::pos2(pos_x, pos_y);
        let new_size = egui::vec2(width, ui_height);

        println!("[overlay] sync position={:?}, size={:?}", new_pos, new_size);
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(new_size));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(new_pos));

        self.last_applied_dialog = Some(dialog);
        self.last_applied_scale = Some(pixels_per_point);
        self.set_overlay_visible(ctx, true);
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
                if self.target_dialog.is_some() {
                    self.target_dialog = None;
                }
                return;
            }

            if self.user_hidden_dialog_hwnd.is_some() {
                println!(
                    "[overlay] release user suppression due to dialog switch: {:?} -> {}",
                    self.user_hidden_dialog_hwnd, info.hwnd
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
                    if Instant::now().duration_since(since) >= Duration::from_millis(HIDE_GRACE_MS)
                    {
                        if self.user_hidden_dialog_hwnd.take().is_some() {
                            println!(
                                "[overlay] release user suppression after dialog session ended"
                            );
                        }
                        self.hide_overlay_by_system(ctx);
                    } else {
                        ctx.request_repaint_after(Duration::from_millis(UI_TICK_MS));
                    }
                }
            }
        }

        if self.target_dialog.is_some() && self.pending_none_since.is_some() {
            ctx.request_repaint_after(Duration::from_millis(UI_TICK_MS));
        }

        // Finalize hide even when `None` is only sent once.
        if let Some(since) = self.pending_none_since
            && Instant::now().duration_since(since) >= Duration::from_millis(HIDE_GRACE_MS)
        {
            if self.user_hidden_dialog_hwnd.take().is_some() {
                println!("[overlay] release user suppression after grace timeout");
            }
            self.hide_overlay_by_system(ctx);
        }
    }
}

impl eframe::App for PathWarpApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        self.sync_dialog_state_from_channel(ctx);

        if let Some(dialog) = self.target_dialog {
            self.place_overlay_for_dialog(ctx, dialog);
            crate::ui::window::render(ctx, self);
        } else {
            self.set_overlay_visible(ctx, false);
            egui::CentralPanel::default().show(ctx, |_| {});
        }

        ctx.request_repaint_after(Duration::from_millis(UI_TICK_MS));
    }
}
