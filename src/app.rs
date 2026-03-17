// 应用程序状态与生命周期管理

pub struct PathWarpApp {
    pub paths: Vec<String>,
    pub search_query: String,
    pub selected_index: usize,
}

impl PathWarpApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            paths: crate::os::explorer::get_open_windows(),
            search_query: String::new(),
            selected_index: 0,
        }
    }
}

impl eframe::App for PathWarpApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        crate::ui::window::render(ctx, self);
    }
}
