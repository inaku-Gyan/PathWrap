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
            
            // Note: egui expects logical points (Points) here, but GetWindowRect returns physical pixels (Pixels).
            // This is largely affected by your monitor's Scaling settings (e.g. 150%, 200%).
            // As a simple workaround to high DPI scaling issues off-screen, we just need to tell eframe to scale it back,
            // or dynamically query zoom factor. Egui points = Pixels / pixels_per_point.
            let pixels_per_point = ctx.pixels_per_point();
            
            let pos_x = dialog.x as f32 / pixels_per_point;
            let pos_y = (dialog.y + dialog.height) as f32 / pixels_per_point;
            let width = dialog.width as f32 / pixels_per_point;

            let new_pos = egui::pos2(pos_x, pos_y);
            let new_size = egui::vec2(width, ui_height);

            // println!("DEBUG: Moving window to: pos={:?}, size={:?} (Scale: {})", new_pos, new_size, pixels_per_point);

            // Using Window level to ensure we draw the floating UI as top level of our invisible eframe context,
            // or alternatively adjusting the viewport commands.
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(new_size));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(new_pos));
            
            // To fix eframe visibility issue on Windows, continually ensure visible & on top
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            // eframe window ordering hook
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));

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
