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

1. **代码质量检查**：调用终端执行 `just check --ci`。
2. **自动修复（按需）**：如需修复格式、lint 或 rustc 建议，调用终端执行 `just fix`，然后再次执行 `just check --ci`。
3. **编译与功能检查**：执行 `just build`。
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
- [x] **[阶段一自检工作流]**: just check --ci -> cargo test -> commit (若有修正)

### 阶段二：UI 层 - 界面搭建与交互

**目标**：使用 `eframe` / `egui` 渲染一个悬浮窗，展示资源管理器路径，并支持键盘鼠标交互。

- [x] **Task 2.1**: 完善 `src/app.rs` 的应用状态模型，包含一个 `Vec<String>` 用于存放路径，以及一个过滤用的搜索字符串（`search_query`）。
- [x] **Task 2.2**: 在 `src/ui/window.rs` 中使用 `egui` 构建列表视图（List View）和顶部的输入搜索框。要求支持方向键上下选择和回车确认（目前确认仅先在控制台打印选择的路径）。
- [x] **Task 2.3**: 在 `src/ui/theme.rs` 添加极简的黑色半透明主题配置或系统跟随主题，去除边框（已在 main 里设定 `with_decorations(false)`），支持拖拽与按 ESC 关闭/隐藏 UI。
- [x] **Task 2.4**: 恢复 GUI 为业务内容（替换当前调试占位文案），重新展示路径列表、搜索框、键盘选择与回车确认等核心交互。
- [ ] **[阶段二自检工作流]**: just check --ci -> cargo test -> commit (若有修正)

### 阶段三：OS 操作层 - 系统文件对话框检测与 UI 粘合

**目标**：检测到“打开”/“保存”对话框出现时，弹出我们自定义的 UI，并将其吸附在对话框下方。

- [x] **Task 3.1**: 在 `src/os/monitor.rs` 实现对 Windows 对话框的监测（可考虑 `SetWindowsHookEx` 侦听 `WH_CBT` 钩子，或者轮询/`UIAutomation` 寻找特定类名 `\#32770` 或 `DirectUIHWND`）。
- [x] **Task 3.2**: 获取到对话框句柄 `HWND` 后，解析该对话框的物理屏幕坐标与大小。
- [x] **Task 3.3**: 将目标对话框的坐标通过某种机制（如 `std::sync::mpsc` 通道或 `Arc<Mutex<...>>`）通知 `PathWarpApp`。在应用 `update` 时，调用 `eframe` 给出的 Window API，修改我们自己 egui 窗口的 Size 和 Position，使其紧贴目标对话框的底部。
- [x] **Task 3.4**: 完成悬浮层显隐稳定性封装（`set_overlay_visible` / `hide_overlay`），确保 file dialog 关闭后 GUI 自动隐藏，重新打开后可再次显示。
- [x] **Task 3.5**: 完成后台服务形态窗口配置：窗口始终不出现在 Windows 任务栏（`with_taskbar(false)`），并清理 monitor 层 unsafe 警告。
- [x] **Task 3.6**: 完成低延迟轮询优化：监控频率提升至 30ms，丢失确认与隐藏收敛超时下调，降低体感延迟。
- [x] **Task 3.7**: 修复 ESC 与监听刷新冲突：当 file dialog 仍在时按 ESC，GUI 应保持用户隐藏态，不允许被下一次监听刷新立即重新拉起（需设计 session 级抑制标记与释放时机）。
- [x] **Task 3.8**: 完善显隐状态机：区分“用户主动隐藏”和“系统检测丢失隐藏”，并补充状态转换日志，防止闪现（先消失一瞬又出现）。
- [x] **Task 3.9**: 修复 GUI 位置与高度：当前悬浮层高度偏大，会遮挡 file dialog 下半部分。需调整默认高度、停靠策略与边界约束，确保“紧贴但不遮挡”对话框主体操作区。
- [x] **Task 3.10**: 优化置顶触发策略：取消持续强制置顶，改为仅在焦点切回 file dialog 时执行一次置顶与位置同步，避免长期抢层级。
- [x] **Task 3.12 (新增)**: 调整 GUI 显示前置条件：仅当“目标 file dialog 处于前台焦点窗口”时才显示/保持显示 GUI；即便系统中存在可识别 file dialog，但若其失焦（切到其他窗口），GUI 必须立即隐藏，待该 dialog 再次获得焦点后再恢复显示与停靠。
  - _说明_：该任务与 Task 3.10 不同。3.10 仅约束“置顶触发时机”，3.12 约束“是否显示 GUI 的根条件”。
- [x] **Task 3.13 (新增)**: 补充焦点白名单：当焦点从 file dialog 切到 PathWrap GUI 本身时，GUI 仍需保持显示（避免用户点击搜索框后 GUI 误隐藏）；仅当 file dialog 与 GUI 均失焦时才隐藏。
  - _说明_：3.12 的“仅 file dialog 聚焦显示”在可用性上过严，3.13 将“GUI 自身聚焦”纳入允许显示条件。
- [x] **Task 3.14 (新增)**: 修复 GUI 点击即消失：将“GUI 聚焦”判定从 egui `ctx.input().focused` 改为 Win32 前台窗口进程判定，仅当焦点位于目标 file dialog 或 PathWrap 窗口时显示 GUI。
  - _说明_：避免鼠标点击 GUI 控件时因焦点源判定不稳定导致误隐藏。
- [x] **Task 3.15 (新增)**: 修复“点击 GUI 仍偶发消失”回归：在 file dialog/PathWrap 焦点切换瞬间加入短暂交互宽限，并为显隐判定补充单元测试覆盖点击过渡帧。
  - _说明_：处理 Win32 前台切换与 egui 输入事件不同步导致的一帧误隐藏问题。
- [ ] **Task 3.11 (提醒项，可滞后)**: 验证并记录“同时存在多个 file dialog”时的行为与期望策略（主跟随窗口选择、切换规则、冲突处理）。
- [x] **Task 3.12**: GUI 贴边优化：将悬浮层与 file dialog 下边缘间距调整为 0，确保视觉上紧贴。
- [x] **Task 3.13**: 跟踪延迟优化：优化对话框跟踪刷新策略，消除可感知跟随延迟。
- [ ] **[阶段三自检工作流]**: just check --ci -> cargo test -> commit (若有修正)

### 阶段四：OS 操作层 - 路径注入与切换 (核心魔法)

**目标**：当用户在 UI 中选择了一个路径后，强制修改系统对话框的工作目录。

- [x] **Task 4.1**: 在 `src/os/dialog.rs` 实现注入逻辑。方案A：基于消息传递 `SendMessageAction`，向对话框发送 `CDM_SETFOLDERPATH` 消息，或模拟键盘输入绝对路径后回车（方案B，备用）。
- [x] **Task 4.2**: 在 `src/ui/window.rs` 捕获到用户的“回车确认”或“鼠标双击”动作后，调用 Task 4.1 的注入逻辑，并在成功后隐藏当前 egui UI 窗口。
- [x] **[阶段四自检工作流]**: just check --ci -> cargo test -> commit (若有修正)

### 阶段五：整体验收与后台常驻优化

**目标**：处理全局生命周期，确保 CPU 和内存占用极低。

- [x] **Task 5.1**: 调整应用生命周期逻辑——如果当前没有活跃的文件对话框，`eframe` 停止重绘或直接最小化/隐藏；仅在检测到对话框时唤醒。
- [x] **Task 5.2**: 移除一切临时调试用的控制台输出，整理并规范通过 `log` 和 `env_logger` 输出的信息。
- [x] **Task 5.3**: 评估并实现事件回调化检测方案（`SetWinEventHook`），替换/补充轮询模式；保留轮询作为 fallback，目标是进一步降低显隐延迟与 CPU 占用。
- [x] **Task 5.4**: 实现可开关的 debug 级别日志开关（默认静默）：
  - 默认仅输出 `error`；
  - 支持通过环境变量或配置文件打开 `debug/trace`；
  - 将 monitor / app 的调试输出统一迁移到 `log` 宏，移除 `println!`。
- [x] **Task 5.5 (后续优化)**: GUI 视觉风格优化：统一间距、字号、列表密度与高亮样式，提升可读性与现代感。（见下方 V2 重构记录）
- **Task 5.6 (未来)**: 支持跟随系统浅色/深色模式自动切换 UI 主题，并保留手动覆盖选项。
- **Task 5.7 (未来)**: 优化多桌面（Virtual Desktop）场景下的显示逻辑，避免跨桌面误显示或焦点错位。
- **Task 5.8 (未来的未来)**: 新增设置界面，支持常用开关（含开机自启启用/关闭）与基础行为配置。
- **[阶段五自检工作流]**: just check --ci -> cargo test -> commit (若有修正)

---

AGENT, 执行本计划时请按当前未完成 Task 顺序推进：每完成一个 Task 必须立刻在本文件勾选/标注状态并同步结果；若发现新增需求或衍生任务，需先补充进对应阶段后再继续开发。

---

## 🔧 V2 架构重构记录

V1 落地后暴露三类问题（显隐交互 bug、注入延迟/不稳、界面简陋），根因集中在三处而非整体架构。保留原有 `os/ui/app` 分层与可测试纯函数，做定向重构：

- **非激活悬浮窗**（`src/os/window_ext.rs`、`src/app.rs`）：悬浮窗改为 `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST`，永不抢前台，对话框在交互期间始终保持前台。由此**删除**了 `should_render_overlay`/焦点 grace/`AlwaysOnTop` 切换/进程前台判定等一整套显隐补丁（原 Task 3.7–3.15 的历史包袱），仅保留“对话框是否前台”这一唯一门控。停靠改用 `SetWindowPos` 物理像素直接匹配对话框 DWM 边界，消除逻辑点/DPI 口径不一致造成的贴边缝隙；隐藏改用真正的 `ShowWindow(SW_HIDE)`，替代移到屏幕外的 hack。
- **全局键盘钩子**（`src/os/input_hook.rs`）：非激活窗拿不到键盘焦点，故用 `WH_KEYBOARD_LL`（门控：悬浮条可见且对话框前台）截获打字/导航键送回 UI 线程，其余按键透传给对话框；egui 降级为纯渲染器（移除 `TextEdit`/`request_focus`）。已知限制：`ToUnicodeEx` 逐键翻译不处理 IME 组字。
- **UI Automation 注入**（`src/os/dialog.rs`，替换原 Task 4.1 的 CDM/键盘模拟方案）：`ElementFromHandle` → 评分定位文件名 Edit 与默认按钮 → `ValuePattern::SetValue` + `InvokePattern::Invoke`。同步、无 sleep、无按键模拟、不抢焦点，对现代 `IFileDialog` 稳定。
- **视觉重塑 + 中文字体**（`src/ui/theme.rs`、`src/ui/window.rs`，对应 Task 5.5）：统一深色调色板/间距/圆角，悬浮卡片加描边+阴影与对话框脱开；加载系统微软雅黑（`msyh.ttc`）以正确显示中文路径。
- **卫生**：移除未用依赖 `lazy_static`/`parking_lot` 与空 `build.rs`；`logging.rs` 接入 `RUST_LOG`（默认 error）；裁剪未用 `windows` features（`Win32_UI_Controls`、`Win32_System_Com_StructuredStorage`）；新增 `raw-window-handle`、`Win32_System_LibraryLoader`、`Win32_UI_TextServices`。全树通过 `cargo clippy --all-targets --all-features -- -D warnings`。

---

## 🔧 V3：点击即消失根治 + TDD 测试体系

V2 落地后暴露致命回归：**点击悬浮条后 GUI 立即消失且不再出现**。定向排查确认根因单一——点击误激活了悬浮窗，抢走对话框前台。据此根治并补齐自动化测试。

- **根因**：`apply_overlay_ex_styles` 应用 `WS_EX_NOACTIVATE` 后的 `SetWindowPos` **漏了 `SWP_FRAMECHANGED`**，扩展样式未即时生效，点击仍激活悬浮窗 → 对话框失去前台 → 门控隐藏。经实测 `SW_HIDE` **不会**饿死 eframe 事件循环（重开仍能唤醒），故“永不再现”是抢焦的下游症状，而非独立 bug。
- **窗口层修复**（`src/os/window_ext.rs`）：`SetWindowPos` 补 `SWP_FRAMECHANGED`；新增 `install_noactivate_subclass` 子类化窗口过程，对 `WM_MOUSEACTIVATE` 硬性返回 `MA_NOACTIVATE`（点击永不激活的第二重保证）；`hide()` → `park()`：仅移屏幕外、保持 `WS_VISIBLE`，去掉 `SW_HIDE`。
- **纯控制器状态机**（新增 `src/core/`，对应架构升级）：`Controller::step(env, event) -> Vec<Effect>` 集中所有显隐/停靠/注入/钩子门控/去抖/抑制决策；时间与前台经 `Env` 注入，可确定性单测。新增**前台丢失 150ms 去抖**吸收瞬时抖动。`app.rs` 退化为薄壳（收事件→step→执行 Effect），`window.rs` 转纯渲染器（读控制器快照、鼠标交互回传 `UiEvent`）。`DialogInfo`/`KeyAction` 上移到 `core::types`。
- **依赖升级**：egui/eframe `0.27 → 0.35`、windows `0.54 → 0.62`（eframe 0.35/wgpu 生态要求），引入官方 UI 测试框架 `egui_kittest`。
- **测试金字塔**：① 控制器单测 12 项（去抖/抑制/注入顺序/停靠去重等全分支）；② `window_ext` 子类化确定性单测（`WM_MOUSEACTIVATE→MA_NOACTIVATE`，fix 的权威证明）；③ `egui_kittest` 胶水层 3 项（过滤渲染/点击回传/搜索行）；④ Windows E2E（`tests/e2e.rs` + `src/bin/dialog_host.rs` 驱动真实 `IFileOpenDialog`，`AttachThreadInput` 强制前台、`SendInput` 真实点击、Win32 探针断言），`#[ignore]` 经 `just e2e` 运行。CI 补 `cargo test`；新增 `.github/workflows/e2e.yml`（手动/每日）。
- **已知环境限制**：E2E 的“点击后悬浮条仍停靠”依赖干净桌面——同时运行的 Listary 等会在文件对话框获焦时弹出自己的搜索条抢走前台，导致悬浮条被正常收起；`clicking_overlay_never_activates_it` 已改为只断言“悬浮窗自身永不成为前台”，对此类干扰鲁棒。

### V3 补记：根因订正（点击仍消失）
V3 初版以为根因是漏 `SWP_FRAMECHANGED`，实测（用户手动 + E2E 强断言）发现**仍会点击即消失**。真正根因：**winit 在启动/显示阶段会用自己算出的 `GWL_EXSTYLE` 覆盖我们首次设置的扩展样式，抹掉 `WS_EX_NOACTIVATE`/`TOOLWINDOW`**；单次设置守不住。运行时探针实测：修复前悬浮窗 ex-style 为 `0x00040118`（无 NOACTIVATE），点击后自我激活抢走对话框前台。
- **修复**：在 `app.rs` 每帧幂等**重新断言**扩展样式（`apply_overlay_ex_styles` 位齐则只读不写），子类化与首次 park 各只做一次。修复后 ex-style 稳定为 `0x08040198 [NOACTIVATE|TOOLWINDOW|TOPMOST]`，点击后前台稳留对话框。
- **渲染器**：切到 glow（OpenGL）。wgpu 的 Windows HWND surface 只报告不透明 `CompositeAlphaMode`，透明窗的透明像素会渲染成黑；glow 经 DWM 合成透明，恢复悬浮卡片的圆角与阴影，并顺带消除无关第三方 Vulkan 层的 loader 报错。
- **测试订正**：E2E `clicking_overlay_keeps_it_docked_and_dialog_foreground` 恢复强断言（点击后①悬浮窗不自我激活②对话框仍前台③悬浮条仍停靠）；此前弱断言在 Listary 运行时会假绿（前台被 Listary 抢走，掩盖了自我激活），是本 bug 漏网的原因。新增 `diagnose_overlay_activation` 诊断用例（运行时 dump ex-style 与点击后前台归属）。

### 待办（未来）
- **Task 5.6**：跟随系统浅色/深色主题。
- **Task 5.7**：多虚拟桌面显示逻辑。
- **Task 5.8**：设置界面（开机自启等）。
- **IME 组字**：`ToUnicodeEx` 逐键翻译不支持中文输入法组字筛选。

> 仍待办：Task 3.11（多 file dialog 并存策略）、Task 5.6（跟随系统浅/深色）、Task 5.7（多桌面）、Task 5.8（设置界面）。V2 交互/注入/停靠需在 Windows 真实对话框上手动验证。
