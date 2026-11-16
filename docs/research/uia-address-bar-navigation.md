# 地址栏 UI Automation 无模拟输入导航可行性研究

研究日期：2026-11-17。对应研究票：[Investigate address-bar navigation using UI Automation without simulated input](https://github.com/inaku-Gyan/PathWrap/issues/63)。本文仅调查公开契约与源代码；未启动应用、检查真实 UIA 树、运行 GUI/E2E 或更改产品代码。

## 结论与证据边界

**公开 UI Automation（UIA）契约不足以确认：PathWarp 能在现有 Open/Save 对话框中，仅用地址栏完成任意文件夹跳转（folder jump）。** UIA 提供发现元素、设置字符串与执行控件动作的接口，但这些接口没有组成一个通用于文件对话框的“切换地址栏编辑模式 → 写入路径 → 提交导航”契约。控件模式（control pattern）表达提供程序（provider）实际暴露的能力，模式还可以随状态改变；不能从外观或控件类型反推出完整操作链。[UIA control patterns][patterns]

本次查阅的 Microsoft 文档没有建立固定的文件对话框地址栏元素树、稳定的地址栏 AutomationId，或必定存在的 Go/导航动作。以下结论是**文档证据不足，需有限原型取证**，不是“UIA 路线不可能”，也不是已验证的兼容性承诺。当前没有本研究采集的运行时证据证明任一目标对话框暴露所需 Go 动作。

地址栏 UIA 仍可作为原生文件夹 API 之后的候选路线。进入产品设计前，必须在明确的对话框/Windows 版本范围内，分别证明编辑模式可达、真正提交动作可调用，以及目标文件夹确实到达。缺少任一环节就应把该提供程序标记为未支持/未验证，不能补上 Enter、鼠标动作、剪贴板或最终 Open/Save 确认。本报告不决定焦点快捷键设计；原生路线另见 [Investigate native navigation of another application's file dialog](https://github.com/inaku-Gyan/PathWrap/issues/62)。

## 文档实际保证了什么

| 环节 | Microsoft 公开契约 | 本问题仍需取证的部分 |
| --- | --- | --- |
| 发现目标 | `IUIAutomation::ElementFromHandle` 取得指定 HWND 的 UIA 元素；UIA 树支持按范围与视图遍历。[ElementFromHandle][from-handle]、[UIA tree][tree] | 对话框的真实地址栏、文件名框、搜索框、文件视图、文件类型控件分别是哪一个元素。 |
| 面包屑与编辑态 | UIA 树会动态添加、移除或移动元素；ComboBox 的示例结构可有 0 或 1 个 Edit，只有允许任意文本输入时才要求该编辑部分。[UIA tree][tree]、[ComboBox control type][combo] | 面包屑（breadcrumb）是否提供切换到可编辑地址的动作；切换后 Edit 是否出现、替换，或始终存在。不得把“面包屑 → Edit → Go”画成已经观察到的真实树。 |
| 设焦点 | `IUIAutomationElement::SetFocus` 设置该元素的键盘焦点。[SetFocus][set-focus] | 给面包屑或容器设焦点是否同时进入地址编辑态。该方法没有承诺此状态转换。 |
| 编辑路径 | 对字符串 Edit，Value 模式是相应契约；`ValuePattern::SetValue` 设置元素值，前提是启用且非只读。[Edit control type][edit]、[SetValue][value-set] | 哪个元素具有可写 Value；设置后是否出现/启用导航动作、是否触发提供程序内部的待提交状态。 |
| 展开控件 | ExpandCollapse 负责显示/隐藏内容；组合控件可能由父元素提供模式，子元素不必提供；子元素还可能按需生成。[ExpandCollapse][expand] | `Expand` 是否只展开历史/位置列表，还是另有实际模式切换。文档没有把展开等同于“编辑地址”或“提交路径”。 |
| 执行导航 | `Invoke` 执行**该控件的关联动作**，例如按钮动作；设计目标是一个明确动作。[Invoke API][invoke-api]、[Invoke pattern][invoke] | 是否存在语义确认为“导航到当前输入地址”的控件/动作。支持 Invoke 本身不表示“向旁边 Edit 发送 Enter”，也不表示目录已经切换。 |
| 兼容 MSAA | LegacyIAccessible 暴露原生 MSAA 服务器；`DoDefaultAction` 执行该元素的 MSAA 默认动作。[LegacyIAccessible][legacy]、[DoDefaultAction][default-action] | 只有实际暴露该模式且默认动作语义已确认时才是候选。不能假定所有 UIA 元素都支持它，或默认动作就是提交地址。 |

### 发现必须限定到当前目标对话框

建议的发现过程是从已确认的目标文件对话框（target file dialog）HWND 建立 UIA 根，记录其后代的 ControlType、AutomationId、ClassName、FrameworkId、Name、HelpText、LabeledBy、启用/只读/离屏状态、支持的模式及父子关系。对比初始态、允许的 UIA 模式调用之后、`SetValue` 之后的快照；必要时比较 raw/control 两种视图。raw view 与底层框架有关，content view 会省略一些交互结构，不能用一个视图中没有 Edit 证明绝无 Edit。[ElementFromHandle][from-handle]、[UIA tree][tree]

AutomationId 只要求同级元素间唯一，可缺失，而且 Microsoft 明确警告不要假定其他应用的值、不要依赖跨版本/构建稳定性；ClassName 也取决于提供程序。因而，应将 ID、层级、标签、模式和已确认的功能一起用作某一兼容性范围内的识别证据，不能把某个本地化名称、数字 ID 或遍历到的第一个 Edit 当作通用地址栏选择器。[Automation element properties][properties]

这里的“面包屑”是待识别的显示/交互角色，不是本文确认的 UIA ControlType。其子项可能表示祖先位置；即使其中某项可 Invoke，动作也可能是打开该位置，而非让任意路径进入编辑态。必须依据真实提供程序的语义和动作结果分类，不能遍历所有按钮试着调用。[Invoke pattern][invoke] 这是对模式契约的工程推断，尚无本研究的运行时树可以验证。

### 写文本与提交导航必须分开

`SetValue` 的已记录语义是设置值，接口只有取值、只读状态和设值能力，没有 Commit/Navigate 方法。于是 `SetValue(target)` 返回 `S_OK`、随后 `CurrentValue == target`，只能作为文本写入证据；不能用它证明文件夹到达。某个提供程序可能因设值自动导航，也可能等待独立动作，这两种行为均需取证。[ValuePattern interface][value]、[SetValue][value-set]

同理，Edit 与 ComboBox 的标准模式要求没有规定一个通用地址提交方法。若真实树出现独立 Go 控件，只有确认其目标是地址导航、并确认其 Invoke 或 MSAA 默认动作实际执行此导航后，才可使用。一个看起来像箭头的控件可能有其他职责；地址栏旁最终 Open/Save 按钮始终不属于本方案可调用的提交动作。[Edit control type][edit]、[ComboBox control type][combo]、[Invoke API][invoke-api]

LegacyIAccessible 是 MSAA→UIA 代理提供的桥接模式，文档明确说原生 UIA 控件不提供该模式。`DoDefaultAction` 的“默认”指元素自身的 MSAA 默认动作，不是对话框默认按钮，也不是万能的键盘回车。若发现它，需记录 CurrentDefaultAction、Role 与关联元素，再在受控对话框中验证动作；`LegacyIAccessible::SetValue` 同样不能凭名称升级为导航提交。[LegacyIAccessible][legacy]、[DoDefaultAction][default-action]

## 如何证明跳转、对话框存活与状态保留

下表是后续原型的**建议验收证据**，不是 UIA 自带的事务/成功保证。所有证据应关联到本次请求及同一个目标对话框实例；用户已关闭/替换对话框、切到另一目标或发起较新请求时，旧结果不应被接受。

| 要验证的事实 | 独立观察方法 | 不能单独作为证明的信号 |
| --- | --- | --- |
| 实际文件夹到达 | 在受控宿主中，用宿主持有的 `IFileDialog::GetFolder` 取得当前文件夹，与目标 Shell 项/文件夹身份比较；把 `OnFolderChange` 当作重新查询触发器，并排除打开对话框时的初始通知。[GetFolder][get-folder]、[OnFolderChange][folder-change] | 刚写入的地址字符串、Value/TextChanged、Invoke 成功或 Invoked 事件。 |
| 外部对话框的 UIA 可观察到达 | 发现能表达**已提交位置**的只读状态，并结合文件视图实际内容更新；受控夹具使用 A/B 两个含不同唯一子文件夹的目录，使文件过滤器不会隐藏验证标记。用“只 SetValue、不调用导航”作负对照，证明观察信号不是输入回显。 | 编辑框目标文本；同名末级文件夹的显示名；单一结构变化事件。若没有可靠已提交位置来源，只能报告“观察到视图变化/夹具行为成立”，不能承诺通用路径身份验证。 |
| 对话框仍打开 | 在到达后重新获取/验证原目标根和可交互子树；监测 WindowClosed/结构移除和访问失败，确认不是别的窗口；受控宿主同时确认 `Show` 尚未返回且没有最终文件操作结果。 | `Invoke` 返回前的一次存活检查，或窗口标题仍相似。 |
| 待处理文件名保留 | 写地址前读取真正文件名 Edit 的完整 Value，导航完成后重新发现并读取，要求与原值相等；受控宿主再用 `GetFileName` 交叉验证。[GetFileName][get-filename] | “没有主动写文件名框”这一实现意图。导航或宿主回调仍可能改变它。 |
| 文件类型/过滤器保留 | 记录文件类型 ComboBox/子列表的实际选中项（Selection/SelectionItem，或提供程序确实暴露的 Value），完成后重新读取并比较；受控宿主用 `GetFileTypeIndex` 及宿主已知的类型表验证。[ComboBox control type][combo]、[GetFileTypeIndex][get-filetype] | 文件列表看起来相似；某个本地化标签未变。UIA 未暴露可比较状态时保留性仍是未知。 |

`GetFolder`、`GetFileName`、`GetFileTypeIndex` 在这里仅用于**自己创建并持有接口的测试宿主**，作为 UIA 动作的独立观察器，不是用来驱动导航。本文没有从外部 HWND 获得任意他进程 `IFileDialog` 接口的方案；不能把测试宿主可观察性视作产品已有能力。只读位置、目录标记与文件类型 UIA 状态在实际目标上是否可取得，也是原型结果的一部分。

状态保留失败应记录为该范围未通过，而不是自动接受或默默改回值：自动恢复本身会增加动作、副作用和并发用户编辑冲突。是否需要恢复及如何仲裁，应在后续设计中单独决定。

## 异步完成、元素生命周期与调用边界

Invoke 提供程序约定要求尽快异步返回；客户端 API 也明确说是否阻塞取决于实现。Invoked 事件在某些耗时或需用户交互的情形可以在动作完成前发出，元素还可能在调用后立即从树中消失。因此不能沿用“同步 COM 调用返回 = 导航结束”的解释，也不能在回调中无条件继续使用旧元素。[Invoke API][invoke-api]、[Invoke pattern][invoke]

建议在动作前订阅范围内的结构、Value/Name、焦点、窗口关闭等可用事件，并保存必要诊断快照；事件只唤醒重新查询。Microsoft 说明部分事件即使状态未变也会发出，也警告提供程序不一定发出所有事件。需要在有限期限内重读当前状态，并保留有限轮询来处理缺失事件；超时应表示“未能确认到达”，不能表示“已证明没有导航”。`WaitForInputIdle` 只等待关联进程空闲，也不是目录到达证明。[UIA events overview][events]、[Subscribing to events][event-subscription]、[WaitForInputIdle][idle]

每次模式/视图切换后重新发现目标子元素，处理 `UIA_E_ELEMENTNOTAVAILABLE`、`UIA_E_NOTSUPPORTED`、禁用及超时。先前采集的模式/树不是永久句柄：UIA 树和模式支持可以动态变化；旧元素失效时不能降级为猜测另一个 Edit、键盘/鼠标消息或最终确认。[UIA tree][tree]、[Control patterns][patterns]、[UIA error codes][errors]

Microsoft 对可能触及自身界面的桌面 UIA 客户端建议使用独立、无窗口的 MTA 线程；事件订阅/移除也应由非 UI 的 MTA 线程一致管理。跨 32/64 位的 UIA 辅助对象还可能受创建 apartment 生命周期影响。后续原型应把 UIA 客户端、订阅与元素生命周期放在受控工作线程，避免把探测或可能阻塞的 provider 调用放在 PathWarp 界面线程；这不是本票的运行时代码修改。[Understanding threading issues][threading]

## 焦点与安全限制

`SetValue` 文档列出的前提是启用和可写，没有把“先模拟聚焦/输入”列为要求。是否需要显式 UIA `SetFocus` 才能暴露编辑态，应作为提供程序假设测量；不能把 `SetFocus` 本身当成编辑模式激活动作。`GetFocusedElement` 能查询输入焦点，但其返回时元素可能已经消失，需要处理失败。[SetValue][value-set]、[SetFocus][set-focus]、[GetFocusedElement][get-focus]

原型应分别记录操作前后焦点元素与前台窗口。用户要求成功跳转后回到目标对话框的焦点状态，应在确认到达后另行验证；UIA 模式调用没有承诺 PathWarp 与对话框之间的完整焦点交接。若后续设计使用 `SetForegroundWindow`，Windows 明确允许拒绝前台切换，不能把调用成功意图当作真实焦点状态。[SetForegroundWindow][foreground] Ctrl+G 及失败时焦点归属不由本研究定案。

完整性级别（integrity level, IL）是能力边界：普通应用不能假定可访问提升权限进程的 UI；UIAccess 有签名、安全安装位置和清单等条件，且 Microsoft 说明不应给非辅助技术应用使用。UIAccess 也不是访问系统 IL/UAC 安全桌面的通行证。本路线不应承诺普通权限 PathWarp 可自动化所有提升权限对话框，或建议为此直接开启 UIAccess。UIA 也不支持通过 Run as 以不同用户启动的进程间通信。[Security considerations][security]、[UIA overview][overview]

## 与发布基线的差距

本研究以公开提交 `15936ab734fe016e46c35f723e34496ea24bf66a` 的 [src/os/dialog.rs](https://github.com/inaku-Gyan/PathWrap/blob/15936ab734fe016e46c35f723e34496ea24bf66a/src/os/dialog.rs) 为代码证据：

- `inject_via_uia` 为后代 Edit 打分，优先文件名相关 ID/名称，然后对选中的 Edit 调用 `ValuePattern::SetValue`；其他 Edit 仍可能作为低优先级候选。
- 随后按 ID/名称选择 Open/Save 确认按钮并 Invoke；没有合适按钮时，`fallback_confirm` 发送 `WM_KEYDOWN/WM_KEYUP(VK_RETURN)`。没有调用 SendInput 不等于没有模拟键盘输入，此回退不符合本题约束。
- 它没有专门的地址栏发现、编辑态激活、Go 动作验证、实际文件夹到达验证，或文件名/类型状态快照与恢复。`Injection succeeded` 日志只反映函数路径返回，并不证明目录到达。

因此不能把现有文件名框路径注入改名为“地址栏 UIA”后沿用，也不能把它视作本路线已有运行时证据。此次没有修改该实现。

## 最小后续原型问题

**在一个明确版本的标准 Common Item Open 对话框和 Save 对话框中，是否存在仅通过已暴露 UIA 模式即可到达的地址 Edit，以及真正提交该 Edit 内容的独立导航动作；执行后能否由独立观察证明到达 B、对话框仍开着，且待处理文件名与类型保持不变？**

建议先限定一台 Windows 的具体构建号、位数、界面语言和同用户/同 IL 的受控宿主。宿主在开始前设置有代表性的待处理文件名与非默认文件类型，并独立记录当前文件夹和状态；外部 UIA 客户端仅驱动地址栏。测试样本不代表任意 Windows 10/11、旧式文件对话框或应用自绘选择器；推广前需对每一宣称支持的家族单独取证。

按以下顺序设门槛，任一步失败就结束该样本并报告具体缺口：

1. **只读发现。** 保存初始 raw/control 树、相关属性与模式；区分地址、文件名、搜索、类型和最终确认控件。没有可信定位就不执行动作。
2. **编辑态可达。** 仅调用已确认语义的 UIA 模式或显式 SetFocus，记录前后树及焦点。若只有鼠标空白区、Alt+D/Ctrl+L 等输入途径，本约束下此样本未通过；不实际发送这些输入。若出现 LegacyIAccessible，单独记录其默认动作语义和使用依据。
3. **设值负对照与真实提交。** 设到 B 后先验证仅文本变化是否已导致导航；若未导航，寻找实际暴露且语义明确的 Go 动作并调用相应模式。若没有独立动作、只有 Enter/最终 Open/Save 可提交，就报告“未发现合规提交链”，不伪造提交。记录每次调用的模式、元素、HRESULT、事件与耗时。
4. **独立验收。** 由宿主读取 GetFolder 与文件名/类型，结合 A/B 不同子文件夹的视图证据，确认到达而非回显；确认对话框未完成/关闭。至少分别覆盖非空文件名 Open、Save，和一个不存在目录的失败样本；失败样本不能误报成功或执行最终文件操作。
5. **边界记录。** 记录元素替换、缺失事件、耗时/超时和焦点变化；输出可支持的具体提供程序范围与仍缺失的外部到达观察信号。产品的焦点交接和 Ctrl+G 行为留给相应设计讨论。

该原型需要新的 GUI 操作安排与人工评估。本研究没有运行它，也不把研究票关闭解释为原型已通过。当前可交付决策是：**保留地址栏 UIA 候选，但在提交动作和独立到达证据完成取证前，不把它作为已验证方案。**

[patterns]: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-controlpatternsoverview
[from-handle]: https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation-elementfromhandle
[tree]: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-treeoverview
[combo]: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-supportcomboboxcontroltype
[edit]: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-supporteditcontroltype
[set-focus]: https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationelement-setfocus
[value-set]: https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationvaluepattern-setvalue
[value]: https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nn-uiautomationclient-iuiautomationvaluepattern
[expand]: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementingexpandcollapse
[invoke-api]: https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationinvokepattern-invoke
[invoke]: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementinginvoke
[legacy]: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementinglegacyiaccessible
[default-action]: https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationlegacyiaccessiblepattern-dodefaultaction
[properties]: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-automation-element-propids
[get-folder]: https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifiledialog-getfolder
[folder-change]: https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifiledialogevents-onfolderchange
[get-filename]: https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifiledialog-getfilename
[get-filetype]: https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifiledialog-getfiletypeindex
[events]: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-eventsoverview
[event-subscription]: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-eventsforclients
[idle]: https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationwindowpattern-waitforinputidle
[errors]: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-error-codes
[threading]: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-threading
[get-focus]: https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation-getfocusedelement
[foreground]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow
[security]: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-securityoverview
[overview]: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-uiautomationoverview
