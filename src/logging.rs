const LOG_CONFIG_FILE: &str = "pathwrap.toml";

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
    let config_path = cwd.join(LOG_CONFIG_FILE);
    let content = std::fs::read_to_string(config_path).ok()?;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        let without_inline_comment = trimmed.split('#').next().unwrap_or("").trim();
        if without_inline_comment.is_empty() {
            continue;
        }

        let Some((key, value)) = without_inline_comment.split_once('=') else {
            continue;
        };

        if key.trim() != "log_level" {
            continue;
        }

        let value = value.trim().trim_matches('"');
        if let Some(level) = parse_level_filter(value) {
            return Some(level);
        }
    }

    None
}

pub fn init_logging() {
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
    builder.init();
}
