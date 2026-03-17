use egui::{Color32, Visuals};

pub fn setup_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    // 极简黑色半透明
    visuals.window_fill = Color32::from_rgba_premultiplied(10, 10, 10, 240);
    ctx.set_visuals(visuals);
}
