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
            // Give our UI a fixed height for now
            let ui_height = 200.0;
            let new_pos = egui::pos2(dialog.x as f32, (dialog.y + dialog.height) as f32);
            let new_size = egui::vec2(dialog.width as f32, ui_height);

            // Using Window level to ensure we draw the floating UI as top level of our invisible eframe context,
            // or alternatively adjusting the viewport commands.
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(new_size));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(new_pos));
            
            // To fix eframe visibility issue on Windows, continually ensure visible & focused
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            // ctx.send_viewport_cmd(egui::ViewportCommand::Focus);

            crate::ui::window::render(ctx, self);
        } else {
            // Optimization: if no target dialog, completely hide viewport
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            // Reset state
            self.search_query.clear();
            self.selected_index = 0;
        }
    }
}
