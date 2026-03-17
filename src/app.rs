use crate::os::monitor::DialogInfo;
use std::sync::mpsc::Receiver;

// 应用程序状态与生命周期管理

pub struct PathWarpApp {
    pub paths: Vec<String>,
    pub search_query: String,
    pub selected_index: usize,
    pub dialog_rx: Option<Receiver<Option<DialogInfo>>>,
    pub target_dialog: Option<DialogInfo>,
}

impl PathWarpApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, dialog_rx: Receiver<Option<DialogInfo>>) -> Self {
        Self {
            paths: crate::os::explorer::get_open_windows(),
            search_query: String::new(),
            selected_index: 0,
            dialog_rx: Some(dialog_rx),
            target_dialog: None,
        }
    }
}

impl eframe::App for PathWarpApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        // Read incoming dialog updates
        if let Some(rx) = &self.dialog_rx {
            for msg in rx.try_iter() {
                self.target_dialog = msg;

                // When a dialog is active, force-fetch the latest paths to keep it fresh
                if msg.is_some() {
                    self.paths = crate::os::explorer::get_open_windows();
                }
            }
        }

        // Only render the UI if we have a target dialog
        if let Some(dialog) = &self.target_dialog {
            // Reposition our window to the bottom of the dialog
            // X matches the dialog X, Y matches dialog Y + dialog height, Width matches dialog Width
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                egui::vec2(dialog.width as f32, 200.0), // Give our UI a fixed height for now
            ));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                dialog.x as f32,
                (dialog.y + dialog.height) as f32,
            )));
            // Ensure we're visible and on top
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));

            crate::ui::window::render(ctx, self);
        } else {
            // Hide the window when there is no target dialog
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            // Reset state
            self.search_query.clear();
            self.selected_index = 0;
        }
    }
}
