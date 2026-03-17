use crate::os::monitor::DialogInfo;
use std::sync::mpsc::Receiver;

// 应用程序状态与生命周期管理

pub struct PathWarpApp {
    pub paths: Vec<String>,
    pub search_query: String,
    pub selected_index: usize,
    pub dialog_rx: Option<Receiver<Option<DialogInfo>>>,
    pub target_dialog: Option<DialogInfo>,

    // 用于防抖 (Debounce)
    pub last_applied_dialog: Option<DialogInfo>,
    pub last_applied_scale: Option<f32>,
    pub is_currently_visible: bool,
}

impl PathWarpApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, dialog_rx: Receiver<Option<DialogInfo>>) -> Self {
        Self {
            paths: crate::os::explorer::get_open_windows(),
            search_query: String::new(),
            selected_index: 0,
            dialog_rx: Some(dialog_rx),
            target_dialog: None,
            last_applied_dialog: None,
            last_applied_scale: None,
            is_currently_visible: false,
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
            let mut should_update_viewport = false;
            let pixels_per_point = ctx.pixels_per_point();

            if let Some(last) = &self.last_applied_dialog {
                if last != dialog {
                    should_update_viewport = true;
                }
            } else {
                should_update_viewport = true;
            }
            
            if let Some(last_scale) = self.last_applied_scale {
                if (last_scale - pixels_per_point).abs() > 0.01 {
                    should_update_viewport = true;
                }
            }

            if !self.is_currently_visible {
                should_update_viewport = true;
            }

            if should_update_viewport {
                self.last_applied_dialog = Some(*dialog);
                self.last_applied_scale = Some(pixels_per_point);
                self.is_currently_visible = true;

                // Give our UI a fixed height for now
                let ui_height = 200.0;

                let pos_x = dialog.x as f32 / pixels_per_point;
                // Place it inside the bottom of the dialog instead of completely below it, just to guarantee it's on screen
                let pos_y = (dialog.y + dialog.height - 200) as f32 / pixels_per_point;
                let width = dialog.width as f32 / pixels_per_point;

                let new_pos = egui::pos2(pos_x, pos_y);
                let new_size = egui::vec2(width, ui_height);

                println!("=> Waking Up App! Move to: logic_pos={:?}, logic_size={:?} (Scale: {})", new_pos, new_size, pixels_per_point);
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(new_size));
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(new_pos));
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
                // We'll remove Force Focus for a moment since that might bug out the real file dialog
                // ctx.send_viewport_cmd(egui::ViewportCommand::Focus); 
            }

            crate::ui::window::render(ctx, self);
        } else {
            // Optimization: if no target dialog, completely hide viewport
            if self.is_currently_visible {
                println!("=> Target lost. Hiding window.");
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                self.is_currently_visible = false;
                self.last_applied_dialog = None;
                self.last_applied_scale = None;
            }

            // Reset state
            self.search_query.clear();
            self.selected_index = 0;
            
            // Render an empty panel anyway just to keep eframe happy 
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
                .show(ctx, |_| {});
        }
    }
}
