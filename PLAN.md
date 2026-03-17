# PathWarp 开发计划 (V1)

## 🏗️ 总体设计 (Architecture)

### 📌 核心功能

- **系统文件对话框检测**：静默监听并捕获 Windows 系统文件选择器（打开/保存）的创建事件。
- **Explorer 监控**：调用 Windows COM 接口 (`IShellWindows`)，实时获取所有活动的资源管理器窗口及其路径。
- **路径同步与切换**：点击或敲击回车，快速将系统文件对话框路径无缝切换到选定目录。
- **快速搜索**：界面内支持键盘打字搜索，智能过滤出正在寻找的路径。

### ⚙️ 技术选型

- **语言**: Rust（追求极端性能、稳定性和极低的运行开销）
- **UI层**: `eframe` + `egui` (现代、跨平台、极速、无原生边框限制的最佳方案)
- **系统交互**: `windows` (windows-rs) 官方 crate，实现底层 COM 编程与 Hook/消息机制。

### 🔄 运行模式与交互

后台常驻运行且0开销。只在检测到“系统文件选择器”弹出时，在选择器底部弹出一个悬浮的 egui 面板；对用户体验 0 干扰，选择完成（或输入回车）后，通过系统消息 (`CDM_SETFOLDERPATH` 或注入逻辑) 瞬间切换对话框当前目录，面板自行隐藏重置。

---

## 📌 Agent 核心工作流与铁律 (Global Instructions)

作为负责执行此计划的 Agent，你需要区分 **“单个子任务 (Task)”** 和 **“阶段划分 (Phase/Milestone)”** 的开发节奏：

### 针对每一个小任务 (Task)

1. **编写代码**：实现该 Task 要求的明确功能，不随意发散。
2. **立即提交 (Commit)**：完成后，必须进行一次代码提交。
   - 要求规范的 Commit Message，格式为：`<type>(<scope>): <subject>`。
   - 示例：`feat(os): implement IShellWindows basics` 或 `refactor(ui): extract list rendering logic`。

### 针对每一个阶段完成时 (Phase 结束)

当一个“阶段 (例如 阶段一)”内的所有 Task 均已提交后，**必须执行全局质量自检**，以检查整体代码情况：

1. **代码格式化**：调用终端执行 `cargo fmt`。
2. **Lint 检查与修复**：调用终端执行 `cargo clippy -- -D warnings`，必须修复所有新出现的警告。
3. **编译与功能检查**：执行 `cargo check` 和 `cargo build`。
4. **单元测试（如有）**：执行 `cargo test`。
5. **阶段性修复提交**：如果有格式化或 Lint 修复，提交一个 `fix: phase x self-check resolved` 的统一 Commit。

> **注意：切勿把整个阶段挤在一次大提交里。必须为每个小 Task 先 Commit 记录，阶段末尾再做全局编译/Lint纠错。**

---

## 🗺️ 第一版 (V1) 阶段划分与任务节点

### 阶段一：OS 操作层 - 数据获取 (Explorer 路径抓取)

**目标**：通过 Windows COM 接口获取所有当前打开的资源管理器窗口的路径。

- [x] **Task 1.1**: 在 `src/os/explorer.rs` 中，编写对 `IShellWindows` COM 接口的调用逻辑。
  - _要求_：需调用 `CoInitializeEx` 进行 COM 环境初始化。遍历当前正在运行的 Explorer 实例。
  - _检查点_：将获取到的 `BSTR` / `IShellItem` 等转换为 Rust 的 `String` 数组。
- [x] **Task 1.2**: 完善错误处理与资源释放，并在 `main.rs` 或单独的文件中写一个临时测试用例/函数，`cargo run` 打印输出确保证明能正确拿到你的桌面上正打开的文件夹路径。
- [x] **[阶段一自检工作流]**: fmt -> clippy -> check -> test -> commit (若有修正)

### 阶段二：UI 层 - 界面搭建与交互

**目标**：使用 `eframe` / `egui` 渲染一个悬浮窗，展示资源管理器路径，并支持键盘鼠标交互。

- [x] **Task 2.1**: 完善 `src/app.rs` 的应用状态模型，包含一个 `Vec<String>` 用于存放路径，以及一个过滤用的搜索字符串（`search_query`）。
- [x] **Task 2.2**: 在 `src/ui/window.rs` 中使用 `egui` 构建列表视图（List View）和顶部的输入搜索框。要求支持方向键上下选择和回车确认（目前确认仅先在控制台打印选择的路径）。
- [x] **Task 2.3**: 在 `src/ui/theme.rs` 添加极简的黑色半透明主题配置或系统跟随主题，去除边框（已在 main 里设定 `with_decorations(false)`），支持拖拽与按 ESC 关闭/隐藏 UI。
- [x] **[阶段二自检工作流]**: fmt -> clippy -> check -> test -> commit (若有修正)

### 阶段三：OS 操作层 - 系统文件对话框检测与 UI 粘合

**目标**：检测到“打开”/“保存”对话框出现时，弹出我们自定义的 UI，并将其吸附在对话框下方。

- [x] **Task 3.1**: 在 `src/os/monitor.rs` 实现对 Windows 对话框的监测（可考虑 `SetWindowsHookEx` 侦听 `WH_CBT` 钩子，或者轮询/`UIAutomation` 寻找特定类名 `\#32770` 或 `DirectUIHWND`）。
- [x] **Task 3.2**: 获取到对话框句柄 `HWND` 后，解析该对话框的物理屏幕坐标与大小。
- [x] **Task 3.3**: 将目标对话框的坐标通过某种机制（如 `std::sync::mpsc` 通道或 `Arc<Mutex<...>>`）通知 `PathWarpApp`。在应用 `update` 时，调用 `eframe` 给出的 Window API，修改我们自己 egui 窗口的 Size 和 Position，使其紧贴目标对话框的底部。
- [x] **[阶段三自检工作流]**: fmt -> clippy -> check -> test -> commit (若有修正)

### 阶段四：OS 操作层 - 路径注入与切换 (核心魔法)

**目标**：当用户在 UI 中选择了一个路径后，强制修改系统对话框的工作目录。

- **Task 4.1**: 在 `src/os/dialog.rs` 实现注入逻辑。方案A：基于消息传递 `SendMessageAction`，向对话框发送 `CDM_SETFOLDERPATH` 消息，或模拟键盘输入绝对路径后回车（方案B，备用）。
- **Task 4.2**: 在 `src/ui/window.rs` 捕获到用户的“回车确认”或“鼠标双击”动作后，调用 Task 4.1 的注入逻辑，并在成功后隐藏当前 egui UI 窗口。
- **[阶段四自检工作流]**: fmt -> clippy -> check -> test -> commit (若有修正)

### 阶段五：整体验收与后台常驻优化

**目标**：处理全局生命周期，确保 CPU 和内存占用极低。

- **Task 5.1**: 调整应用生命周期逻辑——如果当前没有活跃的文件对话框，`eframe` 停止重绘或直接最小化/隐藏；仅在检测到对话框时唤醒。
- **Task 5.2**: 移除一切临时调试用的控制台输出，整理并规范通过 `log` 和 `env_logger` 输出的信息。
- **[阶段五自检工作流]**: fmt -> clippy -> check -> test -> commit (若有修正)

---

AGENT, 接到此计划后，请从 **Task 1.1** 开始你的工作。每次完成子步骤，请务必向我通报结果。
