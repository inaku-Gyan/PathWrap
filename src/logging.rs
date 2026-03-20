pub fn init_logging() {
    let mut builder = env_logger::Builder::new();
    // TODO: 读取应用配置中的日志级别设置
    builder.filter_level(log::LevelFilter::Info);
    builder.init();
}
