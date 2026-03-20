#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // 移除 release 时的黑框
pub mod app;
pub mod os;
pub mod ui;
pub mod utils;

fn enable_per_monitor_v2_dpi_awareness() {
    use windows::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
    };

    // Best-effort: if a manifest already set DPI awareness, this call can fail with access denied.
    let result =
        unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    if result.is_err() {
        log::warn!(
            "SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2) failed; continuing with existing DPI context"
        );
    }
}

fn main() -> eframe::Result<()> {
    env_logger::init();
    enable_per_monitor_v2_dpi_awareness();

    let (tx, rx) = std::sync::mpsc::channel();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_transparent(true)
            .with_taskbar(false)
            .with_decorations(false), // 移除原生边框，后续自定义为漂浮窗口
        ..Default::default()
    };

    eframe::run_native(
        "PathWarp",
        options,
        Box::new(move |cc| {
            ui::theme::setup_theme(&cc.egui_ctx);

            let ctx_clone = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                os::monitor::start_monitor(tx, ctx_clone);
            });

            Box::new(app::PathWarpApp::new(cc, rx))
        }),
    )
}
