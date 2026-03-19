# PathWarp

PathWarp 是一个 Windows 桌面应用，用于在系统“打开/保存”文件对话框出现时，快速切换目标目录。

应用通过监听文件对话框状态并展示轻量悬浮层，结合当前已打开的资源管理器路径，帮助你减少手动逐级点选目录的操作成本。

## 功能简介

- 监听系统文件对话框并在合适时机显示悬浮面板
- 读取当前 Explorer 窗口路径并展示为可选列表
- 支持搜索过滤、键盘上下选择与回车确认
- 使用 Rust + egui/eframe + windows-rs 实现

## 开发环境

- Rust stable（建议通过 `rustup` 安装）
- Cargo（随 Rust 安装）
- Windows 10/11（项目依赖 Win32 API，完整构建与运行需在 Windows 上进行）
- 可选：[`just`](https://github.com/casey/just)（用于执行仓库内 `Justfile` 命令）

## 开发流程

1. 安装依赖工具（Rust、Cargo、可选 just）
2. 克隆仓库并进入目录
3. 按需执行格式化、Lint、构建等命令
4. 在 Windows 环境中运行并验证功能

## 脚本命令（Justfile）

项目根目录提供了 `Justfile`，包含以下基础命令：

- `just fmt`：执行 `cargo fmt --all`
- `just fmt --check`：执行 `cargo fmt --all -- --check`
- `just lint`：执行 `cargo clippy --all-targets --all-features`
- `just lint --check`：执行 `cargo clippy --all-targets --all-features -- -D warnings`
- `just build`：执行 `cargo build --all-targets --verbose`
- `just clean`：执行 `cargo clean`
- `just help`：显示命令帮助

> 说明：在非 Windows 环境中，`build` 可能因 Win32 符号链接限制失败；建议在 Windows 环境完成最终构建验证。
