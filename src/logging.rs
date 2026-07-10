pub fn init_logging() {
    // 默认仅输出 error；通过 RUST_LOG 环境变量可打开 debug/trace。
    let env = env_logger::Env::default().default_filter_or("error");
    env_logger::Builder::from_env(env).init();
}
