# PathWarp 开发状态与计划 (V1–V3)

> 本文件的实现状态以当前 `src/`、`tests/` 和 `.github/workflows/` 为准。核心链路已经落地；剩余工作主要是窗口边界处理、输入法组字、系统主题/多桌面/设置能力，以及 Windows 干净桌面上的实机验收。

## 🏗️ 总体设计 (Architecture)

### 📌 核心功能

- **系统文件对话框检测**：通过 `SetWinEventHook` 唤醒加自适应轮询，识别前台 Windows 系统文件选择器（打开/保存）。
- **Explorer 监控**：调用 Windows COM 接口 (`IShellWindows`)，在新对话框会话开始时读取当前打开的资源管理器窗口路径。
- **路径同步与切换**：点击或敲击回车，快速将系统文件对话框路径无缝切换到选定目录。
- **快速搜索**：界面内支持键盘打字搜索，智能过滤出正在寻找的路径。

### ⚙️ 技术选型

- **语言**: Rust（追求极端性能、稳定性和极低的运行开销）
- **UI层**: `eframe` + `egui` (现代、跨平台、极速、无原生边框限制的最佳方案)
- **系统交互**: `windows` (windows-rs) 官方 crate，实现底层 COM 编程与 Hook/消息机制。

### 🔄 运行模式与交互

后台常驻运行；空闲时停止 eframe 重绘，监视器仍以 30 ms（空闲）/ 8 ms（跟踪）轮询并接受 WinEvent 唤醒。检测到前台文件对话框后，在其底部停靠一个非激活的 egui 面板。回车或双击通过 UI Automation 写入路径并确认，面板保持停靠以便继续选择；ESC 才会将当前会话抑制并把面板移到屏幕外。

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
- [x] **Task 1.2**: `explorer.rs` 已完成 COM 初始化/释放、错误分支处理、URL 到本地路径转换、排序和去重；早期的 `cargo run` 临时打印验证代码没有保留，路径通过应用的新会话刷新使用。
- [ ] **[阶段一自检工作流]**: `just check --ci`、`just build`、`cargo test` 需在可用 Cargo 缓存的 Windows 环境复核。

### 阶段二：UI 层 - 界面搭建与交互

**目标**：使用 `eframe` / `egui` 渲染一个悬浮窗，展示资源管理器路径，并支持键盘鼠标交互。

- [x] **Task 2.1**: 状态已集中到 `src/core/controller.rs`：保存路径列表、查询字符串、过滤结果和选中项；`app.rs` 只负责收集事件和执行 `Effect`。
- [x] **Task 2.2**: `src/ui/window.rs` 提供列表和搜索行的纯渲染；方向键、字符、退格、回车和 ESC 由 `WH_KEYBOARD_LL` 转换后交给控制器，回车/双击会触发真实注入，不再向控制台打印路径。
- [ ] **Task 2.3**: 已实现深色透明主题、无装饰窗口和 ESC 隐藏；窗口拖拽与跟随系统主题尚未实现。系统主题跟随属于 Task 5.6，拖拽暂未列入当前实现。
- [x] **Task 2.4**: GUI 已恢复为路径列表、过滤搜索行、选中高亮和鼠标交互，业务状态由控制器驱动。
- [ ] **[阶段二自检工作流]**: `just check --ci`、`just build`、`cargo test` 需在可用 Cargo 缓存的 Windows 环境复核。

### 阶段三：OS 操作层 - 系统文件对话框检测与 UI 粘合

**目标**：检测到“打开”/“保存”对话框出现时，弹出我们自定义的 UI，并将其吸附在对话框下方。

- [x] **Task 3.1**: `monitor.rs` 使用 `SetWinEventHook`（前台/焦点/显示事件）唤醒监视线程，并以轮询作为 fallback；只把前台 `#32770` 且具有文件对话框子窗口结构的窗口作为新目标，避免误匹配普通消息框。
- [x] **Task 3.2**: 通过 `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` 获取物理视觉边界，失败时回退到 `GetWindowRect`，同时读取窗口 DPI。
- [x] **Task 3.3**: 监视线程经 `mpsc` 通道发送 `DialogInfo`；控制器产生 `Dock`，宿主通过 Win32 `SetWindowPos` 以物理像素停靠到对话框下方。
- [x] **Task 3.4**: 当前显隐由控制器的 `Dock`/`Park` 效果驱动；`park` 把窗口移到 `(-32000,-32000)` 并保持 `WS_VISIBLE`，关闭后可重新停靠。
- [x] **Task 3.5**: 完成后台服务形态窗口配置：窗口始终不出现在 Windows 任务栏（`with_taskbar(false)`），并清理 monitor 层 unsafe 警告。
- [x] **Task 3.6**: 空闲轮询 30 ms，跟踪轮询 8 ms；连续 3 次未找到已信任窗口后上报丢失（约 24 ms），控制器再用 120 ms `None` 宽限收敛隐藏。
- [x] **Task 3.7**: 修复 ESC 与监听刷新冲突：当 file dialog 仍在时按 ESC，GUI 应保持用户隐藏态，不允许被下一次监听刷新立即重新拉起（需设计 session 级抑制标记与释放时机）。
- [x] **Task 3.8**: 控制器区分用户 ESC 抑制、前台丢失宽限和监视器确认丢失三类状态，避免闪现；监视器对检测/切换/丢失写入 debug 日志。
- [ ] **Task 3.9**: 当前悬浮层高度为 140 逻辑像素并按 DPI 缩放，停靠间距为 0，位于对话框下方；尚未实现贴边窗口的屏幕边界裁剪。
- [x] **Task 3.10**: 不再每帧重复移动窗口；仅在初次显示、几何变化或重新获得前台后由控制器产生 `Dock`，Win32 层用 `HWND_TOPMOST | SWP_NOACTIVATE` 更新位置。
- [x] **历史任务 3.12–3.15**: 原先的焦点白名单/egui 聚焦/宽限补丁已被 V2/V3 的非激活窗口方案替代。当前代码不再判定 PathWarp 窗口是否前台，只检查目标对话框前台并保留 150 ms 前台丢失宽限。
- [x] **Task 3.11**: 多对话框策略已落实并有控制器测试：新目标只来自前台窗口；已信任窗口短暂失焦时继续跟踪，句柄重建时在已信任会话内扫描并切换，控制器只维护一个活动目标。
- [x] **Task 3.16**: `OVERLAY_GAP = 0`，悬浮层与 DWM 视觉边界下沿紧贴。
- [x] **Task 3.17**: 事件唤醒 + 8 ms 跟踪轮询用于降低移动跟随延迟。
- [ ] **[阶段三自检工作流]**: `just check --ci`、`just build`、`cargo test` 需在可用 Cargo 缓存的 Windows 环境复核。

### 阶段四：OS 操作层 - 路径注入与切换 (核心魔法)

**目标**：当用户在 UI 中选择了一个路径后，强制修改系统对话框的工作目录。

- [x] **Task 4.1**: `dialog.rs` 使用 UI Automation 定位文件名编辑框和默认按钮，调用 `ValuePattern::SetValue` 后 `InvokePattern::Invoke`；找不到合适按钮时回退为向编辑框发送 Enter。旧的 `CDM_SETFOLDERPATH` 和 `SendInput` 方案不再使用。
- [x] **Task 4.2**: 控制器接收键盘回车、单击/双击事件；单击只改变选中项，回车或双击注入路径并保持悬浮条停靠，ESC 才会 Park 并抑制当前会话。
- [ ] **[阶段四自检工作流]**: `just check --ci`、`just build`、`cargo test` 需在可用 Cargo 缓存的 Windows 环境复核。

### 阶段五：整体验收与后台常驻优化

**目标**：处理全局生命周期，确保 CPU 和内存占用极低。

- [x] **Task 5.1**: 控制器空闲时 `needs_tick()` 返回 false，`app.rs` 不再请求 eframe 重绘；窗口保持停在屏幕外而不是最小化或 `SW_HIDE`，监视器事件/轮询会在新对话框出现时请求重绘。
- [x] **Task 5.2**: 生产应用路径使用 `log`/`env_logger`；`dialog_host` 的 `READY` stdout 和 E2E 诊断输出属于测试协议，不是应用调试输出。
- [x] **Task 5.3**: `SetWinEventHook` 作为唤醒机制，30 ms 空闲轮询和 8 ms 跟踪轮询作为 fallback/持续几何跟踪。
- [x] **Task 5.4**: 实现可开关的 debug 级别日志开关（默认静默）：
  - 默认仅输出 `error`；
  - 支持通过 `RUST_LOG` 环境变量打开 `debug/trace`；
  - monitor / app 的日志统一使用 `log` 宏；默认过滤级别为 `error`，没有配置文件读取。
- [x] **Task 5.5 (后续优化)**: GUI 视觉风格优化：统一间距、字号、列表密度与高亮样式，提升可读性与现代感。（见下方 V2 重构记录）
- **Task 5.6 (未来)**: 支持跟随系统浅色/深色模式自动切换 UI 主题，并保留手动覆盖选项（当前固定为深色主题）。
- **Task 5.7 (未来)**: 优化多桌面（Virtual Desktop）场景下的显示逻辑，避免跨桌面误显示或焦点错位。
- **Task 5.8 (未来的未来)**: 新增设置界面，支持常用开关（含开机自启启用/关闭）与基础行为配置。
- **[阶段五自检工作流]**: `just check --ci`、`just build`、`cargo test` 需在可用 Cargo 缓存的 Windows 环境复核。

---

AGENT, 执行本计划时请按当前未完成 Task 顺序推进：每完成一个 Task 必须立刻在本文件勾选/标注状态并同步结果；若发现新增需求或衍生任务，需先补充进对应阶段后再继续开发。

---

## 🔧 V2 架构重构记录

V1 落地后暴露三类问题（显隐交互 bug、注入延迟/不稳、界面简陋），根因集中在三处而非整体架构。保留原有 `os/ui/app` 分层与可测试纯函数，做定向重构：

- **非激活悬浮窗**（`src/os/window_ext.rs`、`src/app.rs`）：悬浮窗改为 `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST`，并在每帧幂等重申扩展样式；`WM_MOUSEACTIVATE` 子类化返回 `MA_NOACTIVATE`。由此**删除**了 `should_render_overlay`/PathWarp 自身前台白名单等焦点补丁，仅保留“对话框是否前台”这一门控和 150 ms 前台丢失宽限。停靠改用 `SetWindowPos` 物理像素匹配对话框 DWM 边界，悬浮层高度仍按 DPI 缩放；隐藏使用 `park` 把窗口移到屏幕外并保持 `WS_VISIBLE`，不使用 `ShowWindow(SW_HIDE)`。
- **全局键盘钩子**（`src/os/input_hook.rs`）：非激活窗拿不到键盘焦点，故用 `WH_KEYBOARD_LL`（门控：悬浮条可见且对话框前台）截获打字/导航键送回 UI 线程，其余按键透传给对话框；egui 降级为纯渲染器（移除 `TextEdit`/`request_focus`）。已知限制：`ToUnicodeEx` 逐键翻译不处理 IME 组字。
- **UI Automation 注入**（`src/os/dialog.rs`，替换原 Task 4.1 的 CDM/键盘模拟方案）：`ElementFromHandle` → 评分定位文件名 Edit 与默认按钮 → `ValuePattern::SetValue` + `InvokePattern::Invoke`；找不到合适按钮时向编辑框发送 Enter。主路径同步、无 sleep，目标是不抢焦点；具体兼容性仍需在 Windows 实机验证。
- **视觉重塑 + 中文字体**（`src/ui/theme.rs`、`src/ui/window.rs`，对应 Task 5.5）：统一深色调色板/间距/圆角，悬浮卡片加描边+阴影与对话框脱开；加载系统微软雅黑（`msyh.ttc`）以正确显示中文路径。
- **卫生**：移除未用依赖 `lazy_static`/`parking_lot` 与空 `build.rs`；`logging.rs` 接入 `RUST_LOG`（默认 error）；裁剪未用 `windows` features（`Win32_UI_Controls`、`Win32_System_Com_StructuredStorage`）；新增 `raw-window-handle`、`Win32_System_LibraryLoader`、`Win32_UI_TextServices`。全树通过 `cargo clippy --all-targets --all-features -- -D warnings`。

---

## 🔧 V3：点击即消失根治 + TDD 测试体系

V2 落地后暴露致命回归：**点击悬浮条后 GUI 立即消失且不再出现**。定向排查确认根因单一——点击误激活了悬浮窗，抢走对话框前台。据此根治并补齐自动化测试。

- **初步根因（已被后续订正）**：`apply_overlay_ex_styles` 应用 `WS_EX_NOACTIVATE` 后曾遗漏 `SWP_FRAMECHANGED`，导致扩展样式可能未即时生效。后续实测确认，winit 还会在启动/显示阶段覆盖扩展样式；因此当前修复是“带 `SWP_FRAMECHANGED` + 每帧重申样式 + `WM_MOUSEACTIVATE` 子类化”的组合。窗口隐藏策略最终统一为 `park`，不依赖 `SW_HIDE`。
- **窗口层修复**（`src/os/window_ext.rs`）：`SetWindowPos` 补 `SWP_FRAMECHANGED`；新增 `install_noactivate_subclass` 子类化窗口过程，对 `WM_MOUSEACTIVATE` 硬性返回 `MA_NOACTIVATE`（点击永不激活的第二重保证）；`hide()` → `park()`：仅移屏幕外、保持 `WS_VISIBLE`，去掉 `SW_HIDE`。
- **纯控制器状态机**（新增 `src/core/`，对应架构升级）：`Controller::step(env, event) -> Vec<Effect>` 集中所有显隐/停靠/注入/钩子门控/去抖/抑制决策；时间与前台经 `Env` 注入，可确定性单测。新增**前台丢失 150ms 去抖**吸收瞬时抖动。`app.rs` 退化为薄壳（收事件→step→执行 Effect），`window.rs` 转纯渲染器（读控制器快照、鼠标交互回传 `UiEvent`）。`DialogInfo`/`KeyAction` 上移到 `core::types`。
- **依赖升级**：egui/eframe `0.27 → 0.35`、windows `0.54 → 0.62`（eframe 0.35/wgpu 生态要求），引入官方 UI 测试框架 `egui_kittest`。
- **测试金字塔**：① 控制器单测 14 项（去抖/抑制/注入顺序/停靠去重/多对话框跟随等）；② `window_ext` 子类化单测 2 项（`WM_MOUSEACTIVATE→MA_NOACTIVATE` 及零句柄）；③ `egui_kittest` 胶水层 3 项（过滤渲染/点击回传/搜索行）；④ Windows E2E 5 项（`tests/e2e.rs` + `src/bin/dialog_host.rs` 驱动真实 `IFileOpenDialog`，其中 2 项是诊断测试，均 `#[ignore]`，经 `just e2e` 运行）。当前仓库只有 `.github/workflows/ci.yml`，没有单独的 E2E workflow。
- **已知环境限制**：E2E 的“点击后悬浮条仍停靠”依赖干净桌面——同时运行的 Listary 等会在文件对话框获焦时弹出自己的搜索条抢走前台，导致悬浮条被正常收起；当前 `clicking_overlay_keeps_it_docked_and_dialog_foreground` 保留对话框前台、悬浮窗不成为前台、悬浮条仍停靠三项强断言，因此运行前需关闭此类工具。

### V3 补记：根因订正（点击仍消失）
V3 初版以为根因是漏 `SWP_FRAMECHANGED`，实测（用户手动 + E2E 强断言）发现**仍会点击即消失**。真正根因：**winit 在启动/显示阶段会用自己算出的 `GWL_EXSTYLE` 覆盖我们首次设置的扩展样式，抹掉 `WS_EX_NOACTIVATE`/`TOOLWINDOW`**；单次设置守不住。运行时探针实测：修复前悬浮窗 ex-style 为 `0x00040118`（无 NOACTIVATE），点击后自我激活抢走对话框前台。
- **修复**：在 `app.rs` 每帧幂等**重新断言**扩展样式（`apply_overlay_ex_styles` 位齐则只读不写），子类化与首次 park 各只做一次。运行时扩展样式应包含 `NOACTIVATE|TOOLWINDOW|TOPMOST`，具体数值可能随 winit/系统样式变化；点击后前台应稳留对话框。
- **渲染器**：切到 glow（OpenGL）。wgpu 的 Windows HWND surface 只报告不透明 `CompositeAlphaMode`，透明窗的透明像素会渲染成黑；glow 经 DWM 合成透明，恢复悬浮卡片的圆角与阴影，并顺带消除无关第三方 Vulkan 层的 loader 报错。
- **测试订正**：E2E `clicking_overlay_keeps_it_docked_and_dialog_foreground` 恢复强断言（点击后①悬浮窗不自我激活②对话框仍前台③悬浮条仍停靠）；此前弱断言在 Listary 运行时会假绿（前台被 Listary 抢走，掩盖了自我激活），是本 bug 漏网的原因。新增 `diagnose_overlay_activation` 诊断用例（运行时 dump ex-style 与点击后前台归属）。

### 待办（未来）
- **Task 5.6**：跟随系统浅色/深色主题。
- **Task 5.7**：多虚拟桌面显示逻辑。
- **Task 5.8**：设置界面（开机自启等）。
- **IME 组字**：`ToUnicodeEx` 逐键翻译不支持中文输入法组字筛选。

> 仍待办：Task 2.3 中的窗口拖拽、Task 3.9 的屏幕边界裁剪、Task 5.6（跟随系统浅/深色）、Task 5.7（多桌面）、Task 5.8（设置界面），以及 IME 组字支持。V2/V3 的交互、注入、停靠和 E2E 仍需在 Windows 干净桌面上手动验证。
