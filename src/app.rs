use crate::os::monitor::DialogInfo;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

const HIDE_GRACE_MS: u64 = 120;
const UI_TICK_MS: u64 = 30;
const OVERLAY_HEIGHT: f32 = 140.0;
const OVERLAY_GAP: f32 = 0.0;

pub struct PathWarpApp {
    pub dialog_rx: Receiver<Option<DialogInfo>>,
    pub target_dialog: Option<DialogInfo>,
    pub pending_none_since: Option<Instant>,

    pub paths: Vec<String>,
    pub search_query: String,
    pub selected_index: usize,

    pub overlay_visible: bool,
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

            overlay_visible: false,
            last_applied_visible: None,
            last_applied_dialog: None,
            last_applied_scale: None,
        }
    }

    pub fn set_overlay_visible(&mut self, ctx: &eframe::egui::Context, visible: bool) {
        self.overlay_visible = visible;

        if self.last_applied_visible == Some(visible) {
            return;
        }

        self.last_applied_visible = Some(visible);

        if visible {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                egui::WindowLevel::AlwaysOnTop,
            ));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(1.0, 1.0)));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                -10000.0, -10000.0,
            )));
            self.last_applied_dialog = None;
            self.last_applied_scale = None;
        }
    }

    pub fn hide_overlay(&mut self, ctx: &eframe::egui::Context) {
        self.target_dialog = None;
        self.pending_none_since = None;
        self.search_query.clear();
        self.selected_index = 0;
        self.set_overlay_visible(ctx, false);
    }

    fn place_overlay_for_dialog(&mut self, ctx: &eframe::egui::Context, dialog: DialogInfo) {
        let pixels_per_point = ctx.pixels_per_point();
        let needs_update = self.last_applied_dialog != Some(dialog)
            || self
                .last_applied_scale
                .map(|s| (s - pixels_per_point).abs() > 0.01)
                .unwrap_or(true)
            || !self.overlay_visible;

        if !needs_update {
            return;
        }

        let ui_height = OVERLAY_HEIGHT;
        let pos_x = dialog.x as f32 / pixels_per_point;
        let pos_y = (dialog.y + dialog.height) as f32 / pixels_per_point + OVERLAY_GAP;
        let width = dialog.width as f32 / pixels_per_point;

        let new_pos = egui::pos2(pos_x, pos_y);
        let new_size = egui::vec2(width, ui_height);

        println!(
            "=> Waking Up App! Move to: logic_pos={:?}, logic_size={:?} (Scale: {})",
            new_pos, new_size, pixels_per_point
        );
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
                        self.target_dialog = None;
                        self.pending_none_since = None;
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
            self.target_dialog = None;
            self.pending_none_since = None;
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

        if self.target_dialog.is_some() {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(UI_TICK_MS));
        }
    }
}
