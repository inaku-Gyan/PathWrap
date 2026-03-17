use crate::app::PathWarpApp;
use egui::{Context, Key};

pub fn render(ctx: &Context, app: &mut PathWarpApp) {
    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        app.hide_overlay(ctx);
        return;
    }

    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_premultiplied(20, 20, 20, 240))
                .inner_margin(10.0),
        )
        .show(ctx, |ui| {
            ui.heading("PathWarp Overlay");
            ui.label("Bound to active system file dialogs.");
            ui.label("ESC: hide current overlay");

            ui.add_space(8.0);
            ui.label("Waiting for file dialog...");
        });

    // Handle background drag to move window without blocking clicks on children
    if ctx.input(|i| i.pointer.primary_down()) && !ctx.wants_pointer_input() {
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
}
