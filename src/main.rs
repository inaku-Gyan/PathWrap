#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // 移除 release 时的黑框
pub mod app;
pub mod os;
pub mod ui;
pub mod utils;

fn parse_level_filter(level: &str) -> Option<log::LevelFilter> {
    match level.trim().to_ascii_lowercase().as_str() {
        "off" => Some(log::LevelFilter::Off),
        "error" => Some(log::LevelFilter::Error),
        "warn" | "warning" => Some(log::LevelFilter::Warn),
        "info" => Some(log::LevelFilter::Info),
        "debug" => Some(log::LevelFilter::Debug),
        "trace" => Some(log::LevelFilter::Trace),
        _ => None,
    }
}

fn read_level_from_config_file() -> Option<log::LevelFilter> {
    let cwd = std::env::current_dir().ok()?;
    let config_path = cwd.join("pathwrap.toml");
    let content = std::fs::read_to_string(config_path).ok()?;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if let Some(raw) = trimmed.strip_prefix("log_level") {
            let raw = raw.trim_start();
            if !raw.starts_with('=') {
                continue;
            }
            let value = raw.trim_start_matches('=').trim().trim_matches('"');
            if let Some(level) = parse_level_filter(value) {
                return Some(level);
            }
        }
    }

    None
}

fn init_logging() {
    let level = std::env::var("PATHWRAP_LOG_LEVEL")
        .ok()
        .and_then(|v| parse_level_filter(&v))
        .or_else(|| {
            std::env::var("RUST_LOG")
                .ok()
                .and_then(|v| parse_level_filter(&v))
        })
        .or_else(read_level_from_config_file)
        .unwrap_or(log::LevelFilter::Error);

    let mut builder = env_logger::Builder::new();
    builder.filter_level(level);
    builder.parse_default_env();
    builder.init();
}

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
    init_logging();
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
