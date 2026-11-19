# 托盘与独立设置窗口的事件循环、生命周期及焦点集成

研究日期：2026-11-19。对应[研究票 #97](https://github.com/inaku-Gyan/PathWrap/issues/97)，为[设置地图 #95](https://github.com/inaku-Gyan/PathWrap/issues/95)和待人工确认的[窗口契约 #98](https://github.com/inaku-Gyan/PathWrap/issues/98)提供事实。本文只读源码和官方文档；没有运行程序、GUI、构建或测试，也没有修改 Windows、自启登记或产品代码。

## 结论与适用版本

当前依赖确实支持“继续运行现有悬浮层根窗口，另开独立设置原生视口（native viewport）”。但 **`ViewportCommand::Focus` 在本项目锁定的 Windows 后端会模拟 Alt 键**，不能直接把它当成满足本票“正常请求激活、不得模拟产品输入”的方案。采用 egui 子视口时，还需要一个经验证的子窗口 HWND 身份适配器；另一条公开 API 路径是自己持有 HWND 的 Win32 非模态设置窗口（modeless window）。二者是集成选项，本文不替 #98 选择产品窗口政策。来源：[egui 视口定义][EG-VIEW]、[egui-winit 的 Focus 分支][EW-FOCUS]、[winit Windows 激活实现][WI-FORCE]、[CreateDialogParamW][MS-DIALOG]。

研究基线是 PathWarp 提交 `01fa41ba1ec23492be544fdaf7e129cf1b3a94c2`。`Cargo.toml` 使用 glow，`Cargo.lock` 实际锁定 `eframe` / `egui` / `egui-winit` / `egui_kittest` **0.37.0**、`winit` **0.30.13**，不是按“最新版本”推断。[项目依赖][PW-CARGO]、[锁文件][PW-LOCK]。

关键库文件读取自本机已安装的对应版本 Cargo 源码，并用 GitHub Contents API 的 blob SHA 与本地 `git hash-object` 比较：eframe `epi.rs`、`native/run.rs`、`native/glow_integration.rs`、`native/epi_integration.rs`，egui `context.rs`、`viewport.rs`，egui-winit `lib.rs`，winit Windows `window.rs`、`event_loop.rs`、`platform/windows.rs` 均一致。下列引用固定到 egui 官方提交 `3c6eb8261c3293ad9c7ea1ed58487e038860ebef` 和 winit `v0.30.13` 对应提交 `e9809ef54b18499bb4f2cac945719ecc2a61061b`；不依赖可漂移的 `main` 分支。[eframe 源码][EF-APP]、[winit 固定源码][WI-FOCUS]。

## 空闲唤醒与根窗口

当前根窗口以 `visible(false)` 创建；初始化 `park()` 仅用 `SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE` 移到屏幕外，**不会把未显示过的根窗口显示出来**。`SWP_SHOWWINDOW` 在 `dock()`，所以首次 dock 前不能假定 ROOT 已有 `WS_VISIBLE`；随后 park 才保留已经显示的状态。无会话时 `Controller::needs_tick()` 为假，应用停止 30 ms UI 心跳；monitor 只有发现／更新／丢失文件对话框时才请求重绘。**因此托盘必须自己发送应用命令并唤醒根视口，不能等待下一次文件对话框事件，也不能把设置绘制放进 `controller.is_visible()` 分支。** 这是现有控制流的集成约束。[启动][PW-MAIN]、[park 与 dock][PW-WINDOW]、[应用外壳][PW-APP]、[monitor][PW-MONITOR]。

0.37 的 `Context::request_repaint_of(ViewportId::ROOT)` 可从保存的 Context 唤醒根；glow 安装的回调把目标视口和时间交给 winit 的 `EventLoopProxy`。尚未创建的设置视口没有对应 native window，首次打开应唤醒已经存在的 ROOT。[Context 重绘 API][EG-REPAINT]、[glow 重绘桥][EF-REPAINT]。

不能把项目注释“`SW_HIDE` 会饿死事件循环”泛化成此版本的完整事实：0.37 的 runner 对已请求重绘的不可见／最小化 Windows 窗口有直接处理路径；而 glow 在 UI 不可见时可只调用根 `App::logic`，不运行 UI pass。`RequestPaintWhileHidden` 是请求一次完整 UI pass 的公开机制。**本票不改变现有 park 架构**；托盘命令接收应在根 logic 可到达的位置，必要时请求一次完整根 pass 来创建设置子视口；不能依靠 ROOT 过去已因文件对话框显示过。这同时覆盖全新进程从未出现对话框以及最小化／遮挡状态，是源码支持的设计建议，尚未做 Windows 实测。[runner 不可见窗口分支][EF-RUN]、[App::logic][EF-APP]、[RequestPaintWhileHidden][EG-HIDDEN]。

建议的消息边界是“native callback → 有序应用命令队列 → ROOT repaint → 更新 settings-open 状态 → 根 pass 声明设置视口”。回调里不直接重入 egui UI、不直接操纵控制器，也不通过持续轮询保持托盘可用。队列、一次打开意图和重绘是不同事件；应避免重复重绘变成重复打开／重复激活。依据是后端使用跨线程 repaint proxy、deferred 视口可独立重绘，以及当前外壳按通道排空外部事件的架构。[glow 重绘桥][EF-REPAINT]、[deferred API][EG-DEFER]、[应用外壳][PW-APP]。

## 0.37 的多窗口 API 与生命周期

| 选项 | 实际 API／约束 | 对本项目的意义 |
| --- | --- | --- |
| 延迟视口（deferred viewport） | `show_viewport_deferred(id, builder, Fn(&mut Ui, ViewportClass) + Send + Sync + 'static)`；独立重绘，状态用通道或 `Arc<Mutex<_>>` 等通信。 | 设置 UI 可独立重绘；设置动作要唤醒根来提交共享配置或应用命令。 |
| 即时视口（immediate viewport） | `show_viewport_immediate(id, builder, FnMut(&mut Ui, ViewportClass))`；主线程运行，父子重绘互相带动。 | 借用现有 app 状态较简单，但会连带运行根 pass，须核对控制器副作用和空闲开销。 |
| egui 内嵌窗口（embedded window） | 不支持多视口或 `embed_viewports == true` 时使用 `egui::Window` 回退。native eframe 的 Windows 路径启用多视口。 | 内嵌到屏幕外根窗口不满足“无文件对话框也能访问独立设置”的契约；不得把此回退报告成成功创建 native 设置窗口。 |

表中 API、闭包和重绘关系来自[deferred／immediate 源码][EG-DEFER]与[视口类别][EG-VIEW]；Windows backend 的 embed 配置见[eframe winit 初始化][EF-CTX]。这里的 parent-child 是 egui 生命周期树，不等同于设置 HWND 必须是 Win32 `WS_CHILD`。[glow 创建视口窗口][EF-CREATE]。

- **打开与复用**：选固定 `ViewportId`，在设置应存在的每个根 pass 调用对应 show API；相同 ID 更新已有视口。不要每次菜单操作生成新 ID；重绘不是“再次激活”授权。[deferred／immediate API][EG-DEFER]。
- **关闭**：子视口 `close_requested()` 只是交给 app 的请求；app 更新 open 状态并停止声明该视口，后端回收原生窗口。deferred 回调关闭状态后要唤醒根；否则独立重绘不等于父状态已收敛。[ViewportInfo 的 Close 约定][EG-INFO]、[glow 视口回收][EF-GC]。
- **根退出**：ROOT 关闭通常结束应用，可以用 `CancelClose` 拦截；设置关闭不应发送 ROOT Close。退出时再释放托盘／原生窗口资源，设置窗口的生命周期不能独立延长已经结束的 eframe app。[Close／CancelClose][EG-CLOSE]、[App::on_exit][EF-APP]。
- **样式隔离**：设置视口应显式指定自己的透明度。glow 在未指定时会继承根的 transparent 设置，而本项目根透明且 `clear_color` 返回透明；不能把悬浮层的工具窗口、置顶、屏幕外停靠和每帧样式修正套到设置窗口。标题栏、任务栏、普通窗口层级及关闭后是否销毁／保留隐藏实例，属于实现／契约中应明确的设置窗口属性。[glow 透明度继承][EF-STYLE]、[现有外壳][PW-APP]、[现有样式][PW-WINDOW]。

### 三种窗口标识不能混用

egui 的 `ViewportId` 是 app 稳定标识；glow 内部另外保存 `ViewportId ↔ winit::WindowId ↔ Window`；Windows HWND 属于具体 native 实例。停止声明后再用同一 ViewportId 打开，不能假定旧 HWND 仍有效。[glow 内部映射][EF-IDS]、[glow 回收][EF-GC]。

`CreationContext::window_handle()` 给出 ROOT。`Frame` 有 `HasWindowHandle` 和 `winit_window()`，但本后端的 `App::ui` / `App::logic` 只在根调用；deferred 子窗口回调只有 `&mut Ui` 和类别，`ViewportInfo` 没有 HWND 字段。把在根取得的 Frame 捕获进 immediate 闭包，不会因此取得设置 HWND；`NativeOptions.window_builder` 是根创建路径上的一次性 builder hook，也不是子窗口创建完成的 handle callback。这些是**当前公开应用层接口的限制**，不是多窗口不受支持。[eframe 应用／Frame API][EF-APP]、[native 回调分发][EF-DISPATCH]、[ViewportInfo][EG-INFO]、[根 builder hook][EF-BUILDER]。

若继续采用 egui native 设置视口，需要明确实现一个 Windows 身份适配层。例如 `EnumThreadWindows` 可枚举指定 UI 线程的非 child HWND；限定本进程／UI 线程、排除已知根／托盘窗口、记录创建代次，并用 `GetWindowThreadProcessId` 验证，能形成一个候选发现方案。线程限定的 CBT 创建／销毁通知也是公开观察机制。**这些 API 不自动提供 egui ID 映射**；不能仅凭标题、首个枚举结果或 HWND 数值猜测对应关系。候选不唯一、重建或已销毁时应返回未知，不发激活请求。此适配方案需要后续针对实际创建顺序的 Windows 验证，本研究没有宣称它已经可靠完成。[EnumThreadWindows][MS-ENUM]、[GetWindowThreadProcessId][MS-WINDOW-ID]、[CBTProc][MS-CBT]、[SetWindowsHookExW][MS-HOOK]。

另一条无需发现第三方 backend 子 HWND 的公开路径是持有自己创建的 Win32 非模态设置窗口，直接保存返回的 HWND；使用原生控件时须在它所属消息循环调用 `IsDialogMessage` 以支持 Tab 等键盘导航。它需要单独处理布局、DPI、主题及无障碍，并通过同一应用命令层提交设置。它可以继续与现有 eframe 悬浮层共存；这不是新开第二个 `run_native`。选择哪条路径留给后续实现评估，不能为了节省适配工作悄悄恢复模拟输入激活。[CreateDialogParamW][MS-DIALOG]、[IsDialogMessageW][MS-DIALOG-MSG]。

## 正常激活及拒绝后的可观察事实

实际调用链是：`ViewportCommand::Focus` → egui-winit `window.focus_window()` → winit Windows `force_window_active()` → `SendInput` 注入 Alt down/up → `SetForegroundWindow`。因此本任务约束下**不能使用该 Focus 命令或直接调用 winit `focus_window()` 作为正常激活路径**。[egui-winit Focus][EW-FOCUS]、[winit focus_window][WI-FOCUS]、[force_window_active 函数体][WI-FORCE]。

新建设置视口可以显式 `with_active(false)`，使第一次显示使用 `SW_SHOWNOACTIVATE`，再对已验证 HWND 在用户这一次打开意图下调用普通 `SetForegroundWindow`；普通初始 active 创建本身不应与上述模拟输入路径混为一谈。该版本在 fullscreen 创建分支还会调用 `force_window_active`，设置窗口不需要 fullscreen。重复打开、隐藏后显示和最小化恢复还要分别审核显示 API 的激活语义，不能假定一开始的 active=false 会永久约束以后所有 show/restore。[builder active][EG-ACTIVE]、[winit 初次显示][WI-SHOW]、[winit fullscreen 分支][WI-CREATE]、[ShowWindow 的各 show 状态][MS-SHOW]。

Windows 即使在进程获得最后输入等条件满足时也可能拒绝前台请求；`SetForegroundWindow` 返回值说明此次请求结果，`GetForegroundWindow` 可记录当下实际 HWND，切换中也可能返回 NULL。项目现有 `window_ext::activate` 正是一次普通请求后读回实际前台的模式，可供已验证设置 HWND 的适配层沿用。egui `ViewportInfo.focused: Option<bool>` 是后续 backend 焦点输入，`None` 表示未知；只有 egui 焦点输入而没有直接 Win32 激活适配层时，不能声称取得此次 Win32 调用的返回结果。[SetForegroundWindow][MS-ACTIVATE]、[GetForegroundWindow][MS-FOREGROUND]、[现有 activate][PW-WINDOW]、[ViewportInfo][EG-INFO]。

可记录“设置已创建／可见”“已发一次用户激活请求”“观察到已聚焦／未聚焦／未知”，不能把未聚焦立即归因于系统拒绝，也不能将“显示成功”说成“获得前台”。拒绝后保持可点击窗口和真实状态，不循环重试、不模拟输入、不用 thread-input 附着或置顶切换绕过前台限制。具体提示文字和观察等待时间交 #98 决定。地图已确认关闭设置不主动切回悬浮层／文件对话框；Windows 自己选择后继活动窗口不等于 app 发起恢复激活，因此验收须同时观察调用记录和真实前台。[前台限制][MS-ACTIVATE]、[地图既定约束](https://github.com/inaku-Gyan/PathWrap/issues/95)。

## 原生托盘入口、重启与清理

项目已经有 `windows` 的 Shell、WindowsAndMessaging 等 feature，可采用原生通知区域（notification area）API；本票不要求引入 tray 库。创建图标使用 `Shell_NotifyIconW(NIM_ADD)`，填 `NOTIFYICONDATAW.cbSize`、回调 HWND、icon ID、`NIF_MESSAGE`、`uCallbackMessage`、图标等；每次 ADD 后用 `NIM_SETVERSION` 设 `NOTIFYICON_VERSION_4`，各 BOOL 返回值都要处理。[项目 feature][PW-CARGO]、[Shell_NotifyIconW][MS-NOTIFY]、[NOTIFYICONDATAW][MS-NOTIFY-DATA]。

| 回调窗口集成方式 | 生命周期与事件循环要求 | 静态可确认的代价 |
| --- | --- | --- |
| 根 HWND 上的 subclass | 在创建该根的线程安装／移除 `SetWindowSubclass`；回调只处理自己的托盘／Shell 消息，其余交 `DefSubclassProc`。 | 最少新增窗口，依赖 ROOT 长活；菜单用根作 owner 会让系统前台 HWND 可能等于 overlay，影响现有会话判定。须管理 native 回调状态和退出顺序。 |
| 独立隐藏 top-level HWND | 可与 eframe 同线程，由已有 winit 消息循环派发；也可自有线程运行 `GetMessage` / `DispatchMessage`，用队列＋ROOT repaint 桥接。 | 独立线程还要就绪通知、停止命令及 join；窗口须在创建线程销毁。普通隐藏顶层窗口可接收 Shell 广播。 |

表中机制来源：[SetWindowSubclass 的线程约束][MS-SUBCLASS]、[winit 派发所有线程消息][WI-LOOP]、[Windows 消息循环][MS-LOOP]、[DestroyWindow][MS-DESTROY-WINDOW]。选择 callback owner 是技术集成选择，但它对导航／菜单期间会话的影响必须在 #98 的契约中显式处理。

`NativeOptions.event_loop_builder` 可以在创建时配置 winit `with_msg_hook`；该 hook 是 dispatch 前观察／拦截消息，并不自动把 native 消息变成 app 命令，也不替代 SendMessage 直接送达 WndProc 的 subclass。因此自有 callback WndProc 或 subclass 已足够时，无需额外 hook。若使用它，只消费自己的消息，避免阻断普通输入／绘制。当前项目没有直接 winit 依赖；使用其扩展 trait 还需显式声明与锁定栈兼容的依赖。[eframe event-loop hook][EF-RUN]、[winit msg_hook][WI-HOOK]、[当前 manifest][PW-CARGO]。

托盘交互与恢复需覆盖这些机制：

1. VERSION_4 回调中 `LOWORD(lParam)` 是事件、`HIWORD(lParam)` 是 16-bit icon ID，坐标在文档定义的 `wParam` 情况下有效；应支持 `WM_CONTEXTMENU` 和键盘选择事件。菜单“设置…”是已经确认的必要入口；单击／双击打开、气泡通知、丰富 hover 面板、暂停／退出等菜单项不是 Windows 为实现该入口强制要求的产品功能。[回调结构][MS-NOTIFY-DATA]、[Shell 回调差异][MS-NOTIFY]、[地图入口约束](https://github.com/inaku-Gyan/PathWrap/issues/95)。
2. `TrackPopupMenuEx` 可用 `TPM_RETURNCMD` 返回选择的命令；取消返回 0。通知图标菜单要求 owner 为前台才能正确在外部点击时消失；使用普通 `SetForegroundWindow(owner)`，菜单结束后按文档 `PostMessage(WM_NULL)`。选中“设置…”之后再执行设置窗口激活，不能在仍有活动菜单时假定满足 SetForegroundWindow 条件。菜单取消的 `NIM_SETFOCUS` 是返回托盘焦点的 Shell 机制，不是关闭设置后切回文件对话框的授权。[TrackPopupMenuEx][MS-MENU]、[TrackPopupMenu 的 WM_NULL 规则][MS-MENU-NULL]、[NIM_SETFOCUS][MS-NOTIFY]、[激活条件][MS-ACTIVATE]。
3. 通过 `RegisterWindowMessage("TaskbarCreated")` 接收 Shell 重建广播，再 ADD＋SETVERSION 恢复图标；不能把 Explorer 重启当成应用重启。**只有 message-only HWND 不够**：它不接收广播。图标可能在 overflow 区，用户决定是否固定可见，不能保证永久出现在任务栏主区域。[Taskbar Creation Notification][MS-TASKBAR]、[message-only window][MS-WINDOW-FEATURES]、[通知区域][MS-AREA]。
4. 正常退出先停止接受新打开命令，`NIM_DELETE` 删除本次 icon，再释放未挂到窗口的菜单、自有 icon 和 callback 状态，最后销毁所属窗口／退出所属线程；共享 icon 不应 `DestroyIcon`。需兼顾 ADD 部分失败和 Shell 已重启情形。具体顺序是基于 API 所有权的设计建议；各资源规则见[Shell 删除图标][MS-NOTIFY]、[DestroyMenu][MS-DESTROY-MENU]、[DestroyIcon][MS-DESTROY-ICON]、[DestroyWindow][MS-DESTROY-WINDOW]。

托盘 GUID 与“同用户仅一个自启登记”是不同身份。Windows 推荐 GUID 方式，但文档也明确 GUID 与二进制路径关联，移动、并排副本有额外约束；不可直接假定所有便携副本共用固定 GUID。`(hWnd, uID)` 也是公开支持的运行实例标识，生命周期内须保持一致。首版若选择 GUID，应另外说明路径移动与多实例的策略；这不要求修改用户注册表来“修复”托盘，也不扩大成应用副本管理功能。[NOTIFYICONDATAW 的 identity 与 Troubleshooting][MS-NOTIFY-DATA]。

## 现有会话、导航与搜索焦点会怎样

下表描述未集成设置前的真实代码，不把设置预先认定为同一会话或无关应用：

| 位置 | 当前判据 | 设置／托盘进入后的待定影响 |
| --- | --- | --- |
| controller 前台宽限 | 前台是目标 dialog 或 `overlay_focused` 才持续停靠；其余前台超过 150 ms park。已接受 dialog 仍存在时不因 park 自动清 query。 | 若设置获得前台而不改模型，overlay 会 park；这只是当前代码结果，不是已经确认的首版政策。 |
| monitor | 新目标只从前台文件对话框发现；已接受目标可继续按原 HWND 跟踪。 | 设置并非新的目标文件对话框；保留原 session、暂停还是结束，需要显式决定。 |
| 导航 fence／UIA guard | 只认可原 session／dialog identity，且前台必须是 request 的 dialog 或 overlay。 | 设置前台会触发取消／拒绝后续修改；仅把设置加入 controller 的“内部窗口”集合不会同步改变 worker guard。 |
| 完成回调／activation | request、session、dialog identity 匹配才更新；只在 dialog／overlay 前台时产生活动窗口恢复效果，执行前再校验。 | 需要防止打开／关闭设置与晚到结果组合产生意外恢复激活；不能解除现有 stale-result 防护。 |
| Ctrl+G scope | 注册仅在可信 dialog／overlay 对处于前台时有效；作用域结束退订，排队旧结果不得对新会话生效。 | 不应因为设置属于本进程就让其输入被当成 overlay Ctrl+G；托盘菜单 owner 若复用根也要核对。 |
| 搜索框 widget 焦点 | 当前 viewport focused 或主题控件操作后，才恢复搜索 `TextEdit` 的 egui 焦点；已有焦点时不重复 request，避免中断 IME。 | egui widget 焦点不同于 Windows 前台；设置控件应使用自己的视口／widget 身份，不要每帧请求原生 overlay 激活。 |

来源分别是[controller][PW-CONTROLLER]、[monitor][PW-MONITOR]、[Navigator::fence][PW-NAVIGATOR]、[UIA guard][PW-UIA]、[app 的二次 activation 校验][PW-APP]、[Ctrl+G][PW-SHORTCUT]、[搜索控件][PW-UI]。这些边界必须一起审查；只修其中一个会形成互相矛盾的窗口归属。

## 留给 #98 的产品契约

地图已经确认：托盘菜单／悬浮层“设置…”打开独立设置，无文件对话框时可访问；保留主题快捷操作；仅用户主动打开设置时请求激活；关闭设置不主动返回 overlay／dialog；逐项立即生效且自动保存、失败在设置窗口显示。本文不重开这些决策。[地图 Notes](https://github.com/inaku-Gyan/PathWrap/issues/95)。

需要在最终 HITL 票明确的剩余问题：

- 打开设置或仅展开托盘菜单时，现有文件对话框 session 是保留并暂停、继续跟踪但 park，还是结束；query／selection 各保留到何时。窗口可见性和 session identity 不能合成一个布尔值。
- 进行中的导航是在设置打开前先取消并等待已退休、立即打开并显示正在收敛状态，还是采取其他明确顺序；若允许继续，须单独证明 worker 的前台与副作用约束，不能仅扩大“内部窗口”白名单。
- 关闭、失焦和重复打开设置是否改变 query／导航状态；新的明确打开动作如何复用／恢复最小化窗口，哪一次允许发一次正常激活请求。关闭不附加任何显式恢复前台动作。
- 设置“未聚焦／未知”的提示、读回时机，以及托盘创建／恢复失败时如何让用户知道入口不可用。Windows 未保证第一次调用能获得前台；不应把拒绝或未知包装成成功。
- settings 生命周期与 overlay 生命周期分开；关闭销毁还是隐藏复用、普通任务栏／Alt+Tab 可达性、位置／大小是否记忆，需要约定。后端支持不是 UX 决策。

这些是本研究揭示的契约问题，不是对 #98 的预先 resolution。持久化／自启状态由并行[研究票 #96](https://github.com/inaku-Gyan/PathWrap/issues/96)覆盖。

## 后续验证能证明什么

| 验证层次 | 可证明 | 不能证明 |
| --- | --- | --- |
| 确定性状态测试（deterministic state tests） | 注入时间／前台类别／session／窗口代次／open-close 意图，验证保留或结束政策、重复打开只一次副作用、过期结果无效、关闭无恢复激活；模拟 ADD／SETVERSION／重启／清理失败与队列有序性。 | 真实 Shell 回调、原生 HWND 映射、系统是否授予前台。 |
| egui 界面测试（UI harness） | 设置当前值、错误／未知状态、控件事件、独立 widget ID，验证点击／键盘不会改写搜索；对 application command adapter 断言意图。 | 本项目当前的 headless Harness 路径不创建真实 Windows 窗口；内嵌回退或 headless Focus 命令通过不证明 native 多窗口／激活。 |
| 经用户许可的 Windows 交互验收 | 全新进程从未出现文件对话框时，首次托盘唤醒；曾 dock 后 park 的空闲唤醒；重复／最小化打开；独立 HWND 生成与失效；菜单鼠标／键盘／取消；Explorer 重启恢复；正常退出清理；dialog／overlay／settings／无关应用之间切换，包含导航进行中及 IME。 | 有限 OS／显示器／输入配置的通过不等于系统永远授予激活；须保留拒绝／未知路径。 |

确定性边界来源于[Controller 的 Env 注入][PW-CONTROLLER]和[导航 guard][PW-UIA]；headless 限制见[kittest Frame 创建][KIT-BUILDER]及[eframe 测试 Frame][EF-APP]。Windows 验收应记录 OS/build、显示器／DPI、窗口 ID／创建代次、激活请求和实际 foreground、worker retirement，以及菜单和 Shell 回调；模拟系统输入只能是经许可的测试驱动，不能成为产品激活兜底。仓库要求每次 `just e2e` 先征求用户许可；本次没有运行。[仓库验证约定][PW-AGENTS]。

## 来源

[PW-CARGO]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/Cargo.toml
[PW-LOCK]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/Cargo.lock#L810-L918
[PW-MAIN]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/src/main.rs#L59-L92
[PW-APP]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/src/app.rs
[PW-WINDOW]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/src/os/window_ext.rs
[PW-CONTROLLER]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/src/core/controller.rs
[PW-MONITOR]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/src/os/monitor.rs#L124-L233
[PW-NAVIGATOR]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/src/os/dialog.rs#L64-L103
[PW-UIA]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/src/os/dialog/uia.rs#L120-L150
[PW-SHORTCUT]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/src/os/shortcut.rs
[PW-UI]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/src/ui/window.rs#L125-L153
[PW-AGENTS]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/AGENTS.md
[EF-APP]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/eframe/src/epi.rs
[EF-RUN]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/eframe/src/native/run.rs#L204-L294
[EF-REPAINT]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/eframe/src/native/glow_integration.rs#L363-L382
[EF-CTX]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/eframe/src/native/winit_integration.rs#L46-L60
[EF-IDS]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/eframe/src/native/glow_integration.rs#L105-L155
[EF-CREATE]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/eframe/src/native/glow_integration.rs#L194-L239
[EF-DISPATCH]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/eframe/src/native/epi_integration.rs#L330-L406
[EF-BUILDER]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/eframe/src/native/epi_integration.rs#L21-L88
[EF-GC]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/eframe/src/native/glow_integration.rs#L1533-L1553
[EF-STYLE]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/eframe/src/native/glow_integration.rs#L1610-L1642
[EG-VIEW]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/egui/src/viewport.rs#L1-L106
[EG-DEFER]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/egui/src/context.rs#L4580-L4676
[EG-REPAINT]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/egui/src/context.rs#L2115-L2156
[EG-INFO]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/egui/src/data/input/viewport_info.rs
[EG-CLOSE]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/egui/src/viewport.rs#L1085-L1094
[EG-ACTIVE]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/egui/src/viewport.rs#L445-L469
[EG-HIDDEN]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/egui/src/viewport.rs#L1222-L1240
[EW-FOCUS]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/egui-winit/src/lib.rs#L2106-L2110
[WI-FOCUS]: https://github.com/rust-windowing/winit/blob/e9809ef54b18499bb4f2cac945719ecc2a61061b/src/platform_impl/windows/window.rs#L1016-L1026
[WI-FORCE]: https://github.com/rust-windowing/winit/blob/e9809ef54b18499bb4f2cac945719ecc2a61061b/src/platform_impl/windows/window.rs#L1537-L1575
[WI-CREATE]: https://github.com/rust-windowing/winit/blob/e9809ef54b18499bb4f2cac945719ecc2a61061b/src/platform_impl/windows/window.rs#L1396-L1406
[WI-SHOW]: https://github.com/rust-windowing/winit/blob/e9809ef54b18499bb4f2cac945719ecc2a61061b/src/platform_impl/windows/window_state.rs#L315-L338
[WI-LOOP]: https://github.com/rust-windowing/winit/blob/e9809ef54b18499bb4f2cac945719ecc2a61061b/src/platform_impl/windows/event_loop.rs
[WI-HOOK]: https://github.com/rust-windowing/winit/blob/e9809ef54b18499bb4f2cac945719ecc2a61061b/src/platform/windows.rs#L199-L248
[KIT-BUILDER]: https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/egui_kittest/src/builder.rs#L293-L319
[MS-NOTIFY]: https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shell_notifyiconw
[MS-NOTIFY-DATA]: https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ns-shellapi-notifyicondataw
[MS-AREA]: https://learn.microsoft.com/en-us/windows/win32/shell/notification-area
[MS-TASKBAR]: https://learn.microsoft.com/en-us/windows/win32/shell/taskbar#taskbar-creation-notification
[MS-WINDOW-FEATURES]: https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#message-only-windows
[MS-MENU]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-trackpopupmenuex
[MS-MENU-NULL]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-trackpopupmenu
[MS-ACTIVATE]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow
[MS-FOREGROUND]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getforegroundwindow
[MS-SHOW]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow
[MS-LOOP]: https://learn.microsoft.com/en-us/windows/win32/winmsg/using-messages-and-message-queues
[MS-SUBCLASS]: https://learn.microsoft.com/en-us/windows/win32/api/commctrl/nf-commctrl-setwindowsubclass
[MS-DESTROY-WINDOW]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-destroywindow
[MS-DESTROY-MENU]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-destroymenu
[MS-DESTROY-ICON]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-destroyicon
[MS-ENUM]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumthreadwindows
[MS-WINDOW-ID]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowthreadprocessid
[MS-CBT]: https://learn.microsoft.com/en-us/windows/win32/winmsg/cbtproc
[MS-HOOK]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowshookexw
[MS-DIALOG]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createdialogparamw
[MS-DIALOG-MSG]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-isdialogmessagew
