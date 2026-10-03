# IME 组字路径过滤研究

研究日期：2026-10-03

本记录对应 GitHub Issue [IME 组字路径过滤研究](https://github.com/inaku-Gyan/PathWrap/issues/33)。目标是在不让非激活悬浮层抢走文件对话框焦点的前提下，判断中文、日文、韩文输入法的预编辑、提交、取消和输入法切换能否安全接入路径过滤。

## 结论

Windows 的 IME 组字数据不在逐键虚拟键翻译层产生。传统 IMM32 应用从拥有输入上下文的目标窗口收到 WM_IME_STARTCOMPOSITION、WM_IME_COMPOSITION 和 WM_IME_ENDCOMPOSITION；TSF 应用从自己的文本上下文收到组合事件。两者都能区分预编辑和最终提交，但都要求事件在对应的窗口线程或 TSF 文本上下文中处理。

PathWarp 当前由非激活 egui 悬浮层和跨进程 WH_KEYBOARD_LL 组成。低层钩子在键盘事件进入目标线程队列前运行，当前代码用 ToUnicodeEx 生成字符并在成功分类后返回非零值吞掉按键。这条路径没有预编辑、候选提交或组字取消信息；吞掉拼音、假名或韩文物理按键还会阻止文件对话框的 IME 收到它们。因此，现有进程内 API 没有一个安全的 IMM32 旁路可以同时保留非激活窗口、拿到外部对话框的预编辑并阻止文本泄漏。

推荐分两步：

1. 先保留 ASCII/直接键盘布局的现有行为；无法确认有可用 IME/TSF 输入宿主时，进入 fail-open，关闭消费钩子并把按键完整透传给文件对话框。这样牺牲悬浮层 IME 搜索，但不会吞掉用户输入或把半成品字符误写入对话框。
2. 要满足产品票的完整 IME 验收，先做一个 TSF 转发原型：在 PathWarp 自己的 COM/TSF 文本上下文中接收键事件，只有 TSF 明确报告已处理时才让低层钩子返回非零，并把组合文本与最终提交分别送入输入状态机。原型必须证明悬浮层仍不成为前台、目标对话框仍保持焦点，并覆盖 Microsoft Pinyin、日文和韩文配置。原型失败时，产品边界应保持 ASCII 加 IME fail-open，不能伪装成完整支持。

## 当前代码事实

- [src/os/input_hook.rs](../../src/os/input_hook.rs) 安装 WH_KEYBOARD_LL。translate_char 调用 ToUnicodeEx，keyboard_proc 对消费的 WM_KEYDOWN/WM_SYSKEYDOWN 返回 LRESULT(1)；其余事件调用 CallNextHookEx。
- [src/app.rs](../../src/app.rs) 只在悬浮层可见且文件对话框前台时启用消费钩子。
- [README.md](../../README.md) 已把 ToUnicodeEx 无法表达 IME 组字列为已知限制。
- Cargo.toml 已启用 Win32_UI_TextServices、Win32_UI_WindowsAndMessaging 和 COM。IMM32 的 Rust 模块还需要 Win32_UI_Input_Ime。

## 事件来源与线程模型

### IMM32 窗口消息

目标 IME-aware 窗口在 WindowProc 中收到 WM_IME_STARTCOMPOSITION；组字变化通过 WM_IME_COMPOSITION 发送。其 lParam 标志可以包含 GCS_COMPSTR（当前预编辑）、GCS_RESULTSTR（最终结果）、GCS_CURSORPOS、GCS_COMPATTR 和 GCS_COMPCLAUSE。窗口应在处理消息时通过 ImmGetCompositionStringW 读取对应数据。没有 GCS 标志的 WM_IME_COMPOSITION 表示当前组字被取消。WM_IME_ENDCOMPOSITION 表示组字结束；WM_IME_NOTIFY 反映候选窗和状态窗变化。结果也可能以 WM_IME_CHAR 送达，未处理的 IME 消息会由 DefWindowProc 转交默认 IME 窗口。

来源：[WM_IME_COMPOSITION](https://learn.microsoft.com/en-us/windows/win32/intl/wm-ime-composition)、[处理 WM_IME_COMPOSITION](https://learn.microsoft.com/en-us/windows/win32/intl/processing-the-wm-ime-composition-message)、[WM_IME_STARTCOMPOSITION](https://learn.microsoft.com/en-us/windows/win32/intl/wm-ime-startcomposition)、[WM_IME_ENDCOMPOSITION](https://learn.microsoft.com/en-us/windows/win32/intl/wm-ime-endcomposition)、[IME 消息](https://learn.microsoft.com/en-us/windows/win32/intl/ime-messages)、[IME 组合字符串值](https://learn.microsoft.com/en-us/windows/win32/intl/ime-composition-string-values)。

这些消息属于接收键盘焦点的目标窗口线程。IMM 文档明确说明，调用线程若不是指定 HWND 或 HIMC 的创建线程，访问会失败并设置 ERROR_INVALID_ACCESS；IMM 也没有跨线程访问句柄的同步机制。因此 PathWarp 的监视线程或低层钩子线程不能可靠地对外部 #32770 文件对话框调用 ImmGetContext、ImmGetCompositionStringW 或 ImmGetOpenStatus 来轮询组字。

来源：[多线程 IME-aware 应用](https://learn.microsoft.com/en-us/windows/win32/intl/developing-ime-aware-multiple-thread-applications)、[ImmGetContext](https://learn.microsoft.com/en-us/windows/win32/api/imm/nf-imm-immgetcontext)、[输入上下文](https://learn.microsoft.com/en-us/windows/win32/intl/input-context)。

### TSF

TSF 的应用模型由 ITfThreadMgr、文档管理器、上下文和文本存储组成。应用可以为自己的文本上下文安装 ITfContextOwnerCompositionSink，接收 OnStartComposition、OnUpdateComposition 和 OnEndComposition。ITfKeystrokeMgr::TestKeyDown/TestKeyUp 先判断 TSF 是否要吃掉按键，随后由 KeyDown/KeyUp 实际转发；只有 eaten 为 TRUE 时，宿主才应阻止按键继续进入应用。

TSF 还提供 ITfActiveLanguageProfileNotifySink::OnActivated 作为活动语言或文本服务切换通知。WM_INPUTLANGCHANGE 对普通布局有用，但 Microsoft 明确提醒 IME profile 变化可能不会发送该消息。

来源：[TSF 线程管理器](https://learn.microsoft.com/en-us/windows/win32/tsf/thread-manager)、[文本存储](https://learn.microsoft.com/en-us/windows/win32/tsf/text-stores)、[组合](https://learn.microsoft.com/en-us/windows/win32/tsf/compositions)、[TestKeyDown](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfkeystrokemgr-testkeydown)、[KeyDown](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfkeystrokemgr-keydown)、[活动语言配置通知](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itfactivelanguageprofilenotifysink)、[OnActivated](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfactivelanguageprofilenotifysink-onactivated)、[WM_INPUTLANGCHANGE](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-inputlangchange)。

TSF 的 sink 观察的是它所拥有的上下文。PathWarp 在自己的线程上创建 ITfThreadMgr，并不会因此取得外部文件对话框线程的 document manager 或 composition sink；目标程序若不参与，就不能把自己的 TSF sink 当作外部窗口的消息监听器。windows-rs 生成的 TSF COM 类型也标记为 !Send 和 !Sync，应留在创建它们的 COM 线程。

### 现有低层钩子与 ToUnicodeEx

LowLevelKeyboardProc 在键盘输入即将投递到线程输入队列时被调用；回调返回非零会阻止事件到达其余钩子链和目标窗口过程。PathWarp 正是利用这一点避免 WM_CHAR 泄漏，但它也因此会截断 IME 需要的原始按键。ToUnicodeEx 只根据虚拟键、扫描码、键盘状态和 HKL 生成 UTF-16 单元；官方文档只描述死键、连字和 Alt+数字等键盘缓冲状态，未提供组字开始、预编辑、候选或结果生命周期。bit 2 仅在 Windows 10 1607 及以后阻止该函数修改键盘状态，也不改变它没有 IME 组合语义这一事实。

来源：[LowLevelKeyboardProc](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc)、[ToUnicodeEx](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-tounicodeex)、[SetWindowsHookExW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowshookexw)。

### 其他可见信号

GetKeyboardLayout 加 ImmIsIME 只能提示某个输入 locale 是否带 IME，不能证明此刻存在组字。UI Automation 或普通 Raw Input 可用于诊断和观察按键，但不提供可靠的预编辑/提交边界，也不能阻止目标窗口收到消息。WM_INPUTLANGCHANGE 适合重置状态，却不能替代 TSF profile 通知。

来源：[ImmIsIME](https://learn.microsoft.com/en-us/windows/win32/api/imm/nf-imm-immisime)、[GetKeyboardLayout](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getkeyboardlayout)、[WM_INPUTLANGCHANGE](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-inputlangchange)。

## API 与当前架构的比较

| 路径 | 能提供的事实 | 所需所有权 | 对当前架构的结论 |
| --- | --- | --- | --- |
| IMM32 | 预编辑、属性/分句、结果、取消、候选窗状态 | 目标窗口 WindowProc 和同线程 HIMC | 只有在目标线程内宿主或桥接；跨进程轮询不可用 |
| TSF composition sink | 组合开始/更新/结束、文本存储变化 | 自有 document manager/context/text store | 最适合完整实现，但必须先验证非激活宿主与焦点关系 |
| TSF keystroke manager | Test/KeyDown/KeyUp 的 eaten 决策 | COM/TSF 线程和已注册的 key sink | 可作为 forwarding prototype 的输入入口 |
| WH_KEYBOARD_LL + ToUnicodeEx | 原始 VK 和直接布局字符 | 安装钩子的线程 | 保留 ASCII/直接布局；不能表达 IME 组字 |
| WM_INPUTLANGCHANGE / ImmIsIME | 布局或 profile 线索 | 目标窗口消息或 HKL | 只用于状态重置/诊断，不是组字事件源 |
| UIA / Raw Input | 观察或诊断 | 目标控件可访问性/设备输入 | 不能独立保证预编辑和不泄漏 |

若改用目标线程的 WH_CALLWNDPROC、WH_GETMESSAGE 或窗口子类化来桥接 WM_IME 消息，必须引入目标进程内代码、IPC、完整位数匹配和安全边界。Microsoft 文档要求跨进程窗口钩子的过程放在 DLL 中，并说明 32 位与 64 位 DLL 不能互相注入；仅观察消息的 CallWndProc 也不足以阻止目标控件接收结果。它不是当前单二进制 PathWarp 的最小改动。

来源：[Hooks 概览](https://learn.microsoft.com/en-us/windows/win32/winmsg/about-hooks)、[使用 Hooks](https://learn.microsoft.com/en-us/windows/win32/winmsg/using-hooks)、[SetWindowsHookExW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowshookexw)。

## 推荐实现与降级边界

### 安全的首版边界

- 直接输入布局继续沿用现有输入状态机。
- 当目标布局带 IME、TSF 初始化失败、输入上下文不明、目标焦点改变或桌面一致性校验失败时，设置 IME 模式为 unavailable：停用消费钩子，所有按键调用 CallNextHookEx，清除未提交的 composition_text；不要用 ToUnicodeEx 产生拼音/假名字符更新 query。
- 该模式可以保留悬浮层的只读显示，但不宣称支持 IME 搜索。用户输入完整地留给文件对话框，避免“过滤层吃掉按键”和“半成品文本进入对话框”两种事故。

### 满足产品验收的原型方向

先建立一个小型 TSF forwarding prototype，而不是直接把 IMM32 查询塞进现有钩子：

1. 在拥有消息循环的 PathWarp UI 线程初始化 COM，创建 ITfThreadMgr、document manager、context 和最小 ITextStoreACP；所有 TSF 对象留在该线程。
2. 为 context 安装 composition sink，维护独立的 composition_text。OnUpdate 只更新悬浮层预编辑显示；结果文本或结束事件才提交到过滤 query；取消清空 composition_text。候选选择、Backspace、Enter、Escape 等先交给 TSF 的 TestKey/Key 方法，只有未被 TSF 吃掉的键才落入现有导航语义。
3. 低层钩子只做门控和转发：将原始 WM_KEYDOWN/WM_KEYUP 形状的 wParam/lParam 送到 TSF；TSF 返回 eaten 为 TRUE 才返回非零，否则立刻透传。不要在组合期间调用 ToUnicodeEx。
4. 验证 TSF forwarding 是否会改变 foreground HWND、窗口激活或 IME 候选窗口归属。若它需要真实输入焦点，原型应记录失败并转向目标线程 native EDIT/IMM32 bridge；在 bridge 完成前继续使用 unavailable 降级。

这是可行性原型，不是已验证的产品承诺。它解决“如何得到组字生命周期”的问题，但还必须证明非激活悬浮层不抢焦点。

## Rust/windows-rs 可用性

windows 0.62.2 已生成：

- windows::Win32::UI::TextServices 下的 ITfThreadMgr、ITfKeystrokeMgr、ITfContextOwnerCompositionSink、ITextStoreACP 等接口；
- windows::Win32::UI::Input::Ime 下的 ImmGetContext、ImmGetCompositionStringW、ImmGetOpenStatus、ImmReleaseContext 和 GCS 常量；
- windows::Win32::UI::WindowsAndMessaging 下的 WM_IME_* 与 WM_INPUTLANGCHANGE 常量。

当前 Cargo.toml 已启用 Win32_UI_TextServices 和 Win32_UI_WindowsAndMessaging；若后续实现 IMM32 bridge，再加 Win32_UI_Input_Ime。IMM32 与 TSF 代码都必须遵守对象所属线程，不能把 HIMC 或 !Send/!Sync COM 接口跨线程放入全局共享状态。

来源：[windows-rs TextServices 模块](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/TextServices/index.html)、[windows-rs Ime 模块](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Input/Ime/index.html)、[windows-rs ITfThreadMgr](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/TextServices/struct.ITfThreadMgr.html)、[windows-rs ITfContextOwnerCompositionSink](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/TextServices/struct.ITfContextOwnerCompositionSink.html)。

## 测试计划

### 可确定性测试

在平台无关输入状态机中覆盖：

- start/update/end(commit)；
- end(cancel) 和没有 GCS 标志的取消；
- 预编辑期间 Backspace、候选导航、Enter/Escape；
- profile/language 切换时先结束并清空旧 composition；
- TSF eaten/pass 与 ASCII 直接输入的门控；
- unavailable 模式不产生 KeyAction、不改变 query。

### Windows 交互验收

Windows 10 和 Windows 11 各验证一次，至少使用 Microsoft Pinyin、日文 IME、韩文 IME 和英文键盘布局。记录系统版本、输入 profile、前台 HWND、目标线程、IME 模式和每个事件的来源。确认预编辑只出现在悬浮层、候选提交后才进入 query、退格/候选/取消符合输入法语义、组合按键不改变 #32770 的编辑控件文本；悬浮层隐藏、对话框失焦、TSF/COM 失败和桌面切换时所有按键都完全透传。

## 对产品票的调整

[支持 IME 组字的路径过滤输入](https://github.com/inaku-Gyan/PathWrap/issues/34)不能直接以当前 WH_KEYBOARD_LL 加 ToUnicodeEx 为实现方案。它需要先通过一个 TSF forwarding prototype 验证非激活焦点模型；原型未通过前，验收应允许并明确记录 ASCII 加 IME fail-open 的安全降级。完整验收仍需包括预编辑可见、提交进入 query、候选/退格/取消/切换 profile，以及组合按键不泄漏到文件对话框。
