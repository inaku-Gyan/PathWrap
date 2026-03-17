// 应用程序状态与生命周期管理

pub struct PathWarpApp {
    // 这里将存放 UI 状态、监听到的路径列表等
}

impl PathWarpApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {}
    }
}

impl eframe::App for PathWarpApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        crate::ui::window::render(ctx, self);
    }
}
