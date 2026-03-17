#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // 移除 release 时的黑框

pub mod app;
pub mod os;
pub mod ui;
pub mod utils;

fn main() -> eframe::Result<()> {
    env_logger::init();

    // 初始化后台监控
    std::thread::spawn(|| {
        os::monitor::start_monitor();
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_always_on_top()
            .with_decorations(false), // 移除原生边框，后续自定义为漂浮窗口
        ..Default::default()
    };

    eframe::run_native(
        "PathWarp",
        options,
        Box::new(|cc| {
            ui::theme::setup_theme(&cc.egui_ctx);
            // eframe 0.27 期望直接返回 Ok 包装的结构体，Box 实际上是不需要的 (因为 eframe::run_native 期望 Box 作为一个参数或者在最新的 0.27 它期望 Result<Box<dyn App>, _> 嘛？不，看报错是 expected `Box<(dyn App + 'static)>` found enum `Result`)
            Box::new(app::PathWarpApp::new(cc)) // 移除 Ok()
        }),
    )
}
