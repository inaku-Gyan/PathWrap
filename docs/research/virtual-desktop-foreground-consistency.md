# 虚拟桌面前台一致性研究

研究日期：2026-10-03

本记录对应 GitHub Issue [虚拟桌面前台一致性研究](https://github.com/inaku-Gyan/PathWrap/issues/31)，目标是为 PathWarp 在 Windows 10/11 的文件对话框与悬浮层提供可验证的虚拟桌面归属判断，并定义切换桌面时的安全收敛方式。

## 结论

Windows 的公开桌面 API 足以完成本票需求，不需要依赖版本易变的私有 Virtual Desktop Manager 接口。`IVirtualDesktopManager` 由 `VirtualDesktopManager` COM 类实现，公开提供窗口当前桌面判断、窗口桌面 GUID 查询和把窗口移动到指定桌面三个能力；Microsoft 文档将最低客户端版本列为 Windows 10 桌面应用，因此 Windows 10 和 Windows 11 都在同一公开 API 支持范围内。[Microsoft Learn: IVirtualDesktopManager](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ivirtualdesktopmanager)

桌面切换可以通过 WinEvent 的 `EVENT_SYSTEM_DESKTOPSWITCH` 观察。该事件只表示活动桌面发生了切换，并不携带目标桌面 GUID；切换事件到达后仍必须重新查询目标窗口和悬浮层的归属。[Microsoft Learn: Event Constants](https://learn.microsoft.com/en-us/windows/win32/winauto/event-constants)

推荐实现为“事件唤醒 + 每次关键动作前重新验证 + 失败关闭”：切换事件立即让控制器停靠（park）悬浮层并关闭键盘钩子；重新发现当前前台文件对话框后，先确认对话框和悬浮层位于同一桌面，再允许停靠和路径注入。任一 COM 创建或查询失败都按未知归属处理，保持悬浮层停靠且禁止注入，直到探测恢复。这是基于公开 API 能力和 PathWarp 当前“不误注入旧对话框”目标得出的安全推断。

## 公开 API 能力

### `IVirtualDesktopManager`

Microsoft 的接口文档列出三个方法：

- `IsWindowOnCurrentVirtualDesktop(HWND, BOOL*)` 判断顶层窗口是否在当前活动桌面。[方法文档](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ivirtualdesktopmanager-iswindowoncurrentvirtualdesktop)
- `GetWindowDesktopId(HWND, GUID*)` 取得顶层窗口所属桌面的 GUID。[方法文档](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ivirtualdesktopmanager-getwindowdesktopid)
- `MoveWindowToDesktop(HWND, REFGUID)` 将窗口移动到指定桌面。[方法文档](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ivirtualdesktopmanager-movewindowtodesktop)

接口文档明确说明：每个窗口属于一个虚拟桌面；隐藏的桌面上的窗口也会被隐藏；应用应避免自动把用户切换到另一个桌面。PathWarp 因此不应移动用户的文件对话框，也不应主动切换用户桌面。[接口备注](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ivirtualdesktopmanager)

`MoveWindowToDesktop` 对 PathWarp 只适合作为后续实现中的可选自有窗口操作：在已经取得当前文件对话框桌面 GUID、并确认目标是当前活动桌面后，可以把自己的悬浮层移动到该桌面；不能把它用于移动用户的文件对话框，也不能把“移动成功”当作归属验证的替代品。

### COM 与 Rust 绑定

Microsoft 的 COM 初始化文档要求每个使用 COM 的线程先调用 `CoInitializeEx`，并在该线程结束前对成功的初始化调用对应的 `CoUninitialize`。[Microsoft Learn: The COM Library](https://learn.microsoft.com/en-us/windows/win32/com/the-com-library)

当前项目使用 `windows` 0.62。Microsoft 维护的 windows-rs 生成文档显示，`windows::Win32::UI::Shell::IVirtualDesktopManager` 暴露上述三个方法；类型标记为 `!Send` 和 `!Sync`，因此 COM 对象应留在创建它的监视线程，不应直接跨线程共享。[windows-rs IVirtualDesktopManager](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Shell/struct.IVirtualDesktopManager.html)

当前 [Cargo.toml](../../Cargo.toml) 已启用 `Win32_UI_Shell`、`Win32_UI_WindowsAndMessaging` 和 `Win32_System_Com`，公开实现路径不需要新增 Windows crate feature。现有代码也已有 `CoInitializeEx`/`CoCreateInstance` 的模式，可分别参考 [src/os/explorer.rs](../../src/os/explorer.rs) 和 [src/os/dialog.rs](../../src/os/dialog.rs)。

### 桌面切换通知

`SetWinEventHook` 可以注册事件范围，注册线程必须拥有消息循环；当前 [src/os/monitor.rs](../../src/os/monitor.rs) 已在独立线程中使用 `GetMessageW`/`DispatchMessageW` 维持消息循环。[Microsoft Learn: SetWinEventHook](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwineventhook)

公开事件表定义 `EVENT_SYSTEM_DESKTOPSWITCH`（值 `0x0020`）为活动桌面已切换。当前代码只注册 `EVENT_SYSTEM_FOREGROUND`、`EVENT_OBJECT_FOCUS` 和 `EVENT_OBJECT_SHOW`，因此实现票需要增加该事件；收到事件后应触发一次立即探测，而不是等待普通轮询。[Microsoft Learn: Event Constants](https://learn.microsoft.com/en-us/windows/win32/winauto/event-constants)

## 与当前 PathWarp 的差距

当前监视器通过前台 HWND 和窗口结构识别文件对话框，将 `DialogInfo`（HWND、矩形、DPI）发送给控制器；控制器目前只用 `foreground_hwnd == dialog.hwnd` 做前台门控。`DialogInfo` 没有虚拟桌面身份，悬浮层停靠/park 也没有桌面归属检查。因此，切换桌面后旧 HWND 仍存活时，现有状态机没有独立的桌面切换信号来清除它。

## 后续实现边界

### 建议的最小路径

1. 在监视线程初始化 COM，并创建一个只在线程内使用的 `IVirtualDesktopManager`。
2. 为 `EVENT_SYSTEM_DESKTOPSWITCH` 增加 WinEvent 唤醒。事件到达时先向控制器发送桌面变化，再重新读取当前前台文件对话框。
3. 在停靠和路径注入前查询目标对话框的桌面 GUID，并查询悬浮层的桌面 GUID；只有两次查询成功且 GUID 相同，才允许显示、启用键盘钩子或注入。
4. 桌面切换、查询失败、窗口句柄失效或 GUID 不一致时，立即 park 悬浮层、关闭键盘钩子、清除待注入目标，并丢弃旧 `DialogInfo`。不要用旧矩形或旧 HWND 继续注入。
5. 当前桌面重新出现前台文件对话框后，重新获取其 GUID。若悬浮层仍在旧桌面，可仅移动自己的悬浮层到该 GUID；移动后再次查询确认一致，再产生 Dock。不要自动切换用户桌面。

### 安全回退

- COM 类创建失败、方法返回失败或 GUID 查询结果未知：按“不在同一桌面”处理，park 并禁用钩子/注入；记录可诊断日志，不猜测桌面归属。
- `EVENT_SYSTEM_DESKTOPSWITCH` 丢失时，现有 8 ms 跟踪轮询仍应在下一次探测中收敛；事件是降低延迟的唤醒，不是唯一正确性来源。
- 不使用 `IVirtualDesktopManagerInternal`、Shell 服务内部接口或按 Windows build 变化的私有 IID 作为首版依赖。公开接口已经覆盖本票的归属判断和自有悬浮层移动需求；私有接口只能另立研究票并承担版本兼容风险。

## 验收调整

实现票[优化多虚拟桌面显示与前台一致性](https://github.com/inaku-Gyan/PathWrap/issues/32)应增加这些验收点：

- Windows 10 和 Windows 11 各验证一次；至少两个虚拟桌面、一个或多个显示器。
- 对话框和悬浮层桌面 GUID 相同才允许 Dock；GUID 查询失败或不一致时悬浮层必须保持 park，键盘钩子关闭，不能向旧对话框注入。
- 使用系统切换桌面的操作触发 `EVENT_SYSTEM_DESKTOPSWITCH` 后，旧会话立即收敛；切回并重新出现文件对话框后，重新探测并恢复 Dock。
- 验证悬浮层自身跨桌面移动只发生在确认目标 GUID 后，用户文件对话框不被移动、不被自动切换桌面。
- 加入单元测试覆盖“桌面变化/未知归属 → Park + 禁用注入”和“重新验证成功 → Dock”；交互 E2E 记录 Windows 版本、构建号、桌面数量、显示器/DPI 和诊断日志。

