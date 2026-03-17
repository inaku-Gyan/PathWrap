use crate::app::PathWarpApp;
use egui::Context;

pub fn render(ctx: &Context, _app: &mut PathWarpApp) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("PathWarp");
        ui.label("这里将显示探测到的资源管理器路径列表。");
    });
}
