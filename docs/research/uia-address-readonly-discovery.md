# UIA 地址控件的只读能力盘点

研究日期：2026-11-19。对应 [研究票 #80](https://github.com/inaku-Gyan/PathWrap/issues/80)，供 [后续调查决策 #79](https://github.com/inaku-Gyan/PathWrap/issues/79) 使用。本轮只阅读官方文档、官方源码和本地绑定，没有运行 GUI、UIA 查询或动作，也没有关闭进程或服务。

## 结论与证据范围

普通原生外部客户端可以对**固定版本的标准控制模式目录（control-pattern catalog）**逐项读取可用性属性，再按需获取模式接口；这些查询不需要先进入编辑态。`IUIAutomationElement::GetCurrentPattern` 获取当时可用的接口，查询接口与调用其动作方法是两回事。[原生 GetCurrentPattern](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationelement-getcurrentpattern)、[UIA 客户端属性读取](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-propertiesforclients)

这能补全已接受原型遗漏的可用性和 Legacy 元数据；**不能由文档推出本机 Common Item Dialog 实现了某个模式，更不能推出可见编辑态或任意目标文件夹提交能力**。已接受源码只查询 Invoke、Value、ExpandCollapse、Legacy 四个模式，地址范围内只读 UIA Value/IsReadOnly、Legacy Role/DefaultAction，且部分 getter 错误经 `.ok()` 丢失。此前同文件夹面包屑 Invoke 的报告仍只观察到 A，隐藏编辑器未变为可见。[固定原型源码](https://github.com/inaku-Gyan/PathWrap/blob/a1daeeec7db641336adac80934053b3765ee1247/src/bin/uia_current_folder_breadcrumb_prototype.rs#L456)、[已接受报告](https://github.com/inaku-Gyan/PathWrap/blob/a1daeeec7db641336adac80934053b3765ee1247/docs/prototypes/uia-current-folder-breadcrumb/README.md)

## 原生客户端应使用的查询

1. 对下节目录的每个 `Is…AvailablePropertyId` 调用 `GetCurrentPropertyValueEx(id, true)`，再用 `IUIAutomation::CheckNotSupported` 检查保留的 NotSupported 值。可用性属性的正常类型为 `VT_BOOL`，默认值为 FALSE；使用 `ignoreDefaultValue=true` 才能区分不支持属性与普通默认 FALSE。[可用性属性](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-control-pattern-availability-propids)、[GetCurrentPropertyValueEx](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationelement-getcurrentpropertyvalueex)、[CheckNotSupported](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation-checknotsupported)
2. **建议首轮以可用性结果为主**，仅对正向可用项调用 `GetCurrentPattern(pattern_id)` 获取 `IUnknown`；只有读取允许的属性时才取得相应的类型接口，也可直接使用匹配 IID 的 `GetCurrentPatternAs`。保留属性查询与接口获取的两份结果：时间不同，接口可能已经失效；获取接口成功也没有执行它的动作。[GetCurrentPattern](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationelement-getcurrentpattern)、[GetCurrentPatternAs](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationelement-getcurrentpatternas)

不要混用以下接口：

| 名称 | 实际边界 |
| --- | --- |
| `AutomationElement.GetSupportedPatterns()` | .NET 托管客户端方法，文档注明逐个查询、开销大；不是 native `IUIAutomationElement` 的同名成员。[托管文档](https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.automationelement.getsupportedpatterns?view=windowsdesktop-10.0) |
| `IRawElementProviderSimple::GetPatternProvider` | provider 实现向 UIA 提供模式的入口，文档允许 unsupported 返回 NULL；不能把该 provider 合约直接当作外部 native client 的接口。[provider 文档](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationcore/nf-uiautomationcore-irawelementprovidersimple-getpatternprovider) |
| `IUIAutomation::PollForPotentialSupportedPatterns` | 虽是 native API，但微软明确仅供 UIA 扫描工具、非普通客户端用途，且不保证之后仍支持返回的模式；本建议不使用。[方法及限制](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation-pollforpotentialsupportedpatterns) |

## 固定目录：35 个标准模式

目录标识：`windows-0.62.2 / windows-rs 32c3144490c016fe496a0aed769bce60987a2e9d`。本地 `Cargo.lock` 锁定 windows/windows-core 0.62.2、windows-result 0.4.1；注册表源码的 `.cargo_vcs_info.json` 指向上述官方提交。已静态核对 Accessibility 模块，正好有 **35 个 `UIA_PATTERN_ID` 模式常量及 35 个对应可用性常量**；现有 Cargo features 已包含 Accessibility、Com、Ole、Variant，可读取这些接口，无需新增依赖。[项目锁文件](https://github.com/inaku-Gyan/PathWrap/blob/17ea89b43d96f09499213f4745be476db885bc0e/Cargo.lock)、[项目 features](https://github.com/inaku-Gyan/PathWrap/blob/17ea89b43d96f09499213f4745be476db885bc0e/Cargo.toml)、[官方绑定固定源码](https://github.com/microsoft/windows-rs/blob/32c3144490c016fe496a0aed769bce60987a2e9d/crates/libs/windows/src/Windows/Win32/UI/Accessibility/mod.rs#L20288)

表中名称按绑定拼写；模式常量为 `UIA_<名称>Id`，可用性常量为 `UIA_Is<名称>AvailablePropertyId`。应显式保存下表，**不是对任意整数区间扫描**，也不涵盖自定义或未来新增模式。微软的两个概览页当前只列 34 项，未列 SelectionPattern2；第 35 项由上述固定官方绑定补齐。[模式标识符概览](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-controlpattern-ids)、[可用性概览](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-control-pattern-availability-propids)

| 名称 | Pattern ID | Availability property ID |
| --- | --- | --- |
| InvokePattern | 10000 | 30031 |
| SelectionPattern | 10001 | 30037 |
| ValuePattern | 10002 | 30043 |
| RangeValuePattern | 10003 | 30033 |
| ScrollPattern | 10004 | 30034 |
| ExpandCollapsePattern | 10005 | 30028 |
| GridPattern | 10006 | 30030 |
| GridItemPattern | 10007 | 30029 |
| MultipleViewPattern | 10008 | 30032 |
| WindowPattern | 10009 | 30044 |
| SelectionItemPattern | 10010 | 30036 |
| DockPattern | 10011 | 30027 |
| TablePattern | 10012 | 30038 |
| TableItemPattern | 10013 | 30039 |
| TextPattern | 10014 | 30040 |
| TogglePattern | 10015 | 30041 |
| TransformPattern | 10016 | 30042 |
| ScrollItemPattern | 10017 | 30035 |
| LegacyIAccessiblePattern | 10018 | 30090 |
| ItemContainerPattern | 10019 | 30108 |
| VirtualizedItemPattern | 10020 | 30109 |
| SynchronizedInputPattern | 10021 | 30110 |
| ObjectModelPattern | 10022 | 30112 |
| AnnotationPattern | 10023 | 30118 |
| TextPattern2 | 10024 | 30119 |
| StylesPattern | 10025 | 30127 |
| SpreadsheetPattern | 10026 | 30128 |
| SpreadsheetItemPattern | 10027 | 30132 |
| TransformPattern2 | 10028 | 30134 |
| TextChildPattern | 10029 | 30136 |
| DragPattern | 10030 | 30137 |
| DropTargetPattern | 10031 | 30141 |
| TextEditPattern | 10032 | 30149 |
| CustomNavigationPattern | 10033 | 30151 |
| SelectionPattern2 | 10034 | 30168 |

## 必须保留的结果分类

每项独立保存目录版本、元素身份、时间、property/pattern ID、是否尝试、HRESULT（可获得时）、VARIANT 类型和值、接口是否为空、getter 结果。不要把所有失败压成 `available=false`。

| 结果 | 可据此陈述的事实 |
| --- | --- |
| `VT_BOOL=true/false` | UIA 在该次读取报告可用/不可用；不是永久支持结论。 |
| `CheckNotSupported=true` | 未得到该属性的受支持值；与正常布尔 FALSE 分开。 |
| 非预期类型、检查失败 | 记 `unknown/type_error` 或原始错误，不强制转换成 false。 |
| `UIA_E_NOTSUPPORTED (0x80040204)` | 文档定义为 provider 明确不支持指定属性或模式。 |
| `UIA_E_ELEMENTNOTAVAILABLE`、`UIA_E_TIMEOUT`、`E_POINTER` 等 | 原样记录失效、超时或其他错误；不能泛化为模式不支持。 |
| 成功 HRESULT + 非空接口 | 在该时刻取得接口；具体 getter 仍可能失败。 |
| 成功 HRESULT + NULL 接口 | 记 `success_null/no_interface`；没有取得可用接口，不推断永久不支持或动作语义。 |
| 未查询、预算终止 | `not_requested` / `incomplete`；不计为 negative。 |

前三行依据 [属性读取与默认值](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-propertiesforclients) 和 [CheckNotSupported](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation-checknotsupported)；错误含义依据 [UIA 错误码](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-error-codes)。其余分类是本研究建议的观测格式，避免超出一次查询所能证明的内容。native GetCurrentPattern 文档没有要求所有 unsupported 情况都必须表现为某一个 HRESULT；不能直接套用 provider-only 的 NULL 合约。

**windows-rs 的空接口陷阱。** 此版本 `GetCurrentPattern` 和 `GetCurrentPatternAs` 在成功 HRESULT 后调用 `Type::from_abi`；接口指针为空时后者返回 `Error::empty()`，其 `code()` 是 `HRESULT(0)`。所以 native `S_OK + NULL` 可以表现为 Rust `Err`、错误码 0；其他成功 HRESULT 也会在该层丢失，不能从 `Err(0)` 反推原始 HRESULT。[生成包装](https://github.com/microsoft/windows-rs/blob/32c3144490c016fe496a0aed769bce60987a2e9d/crates/libs/windows/src/Windows/Win32/UI/Accessibility/mod.rs#L9686)、[Type::from_abi](https://raw.githubusercontent.com/microsoft/windows-rs/32c3144490c016fe496a0aed769bce60987a2e9d/crates/libs/core/src/type.rs)、[Error::empty/code](https://github.com/microsoft/windows-rs/blob/32c3144490c016fe496a0aed769bce60987a2e9d/crates/libs/result/src/error.rs#L80)

两种可实施记录方式：首轮沿用高层绑定即可，把正向项的 `Err(0)` 单列为 `binding-null/unknown`，明确原始成功 HRESULT 与指针未分别保存；可用性仍由前述属性查询给出。若将来需要精确区别 `S_OK/NULL`，再使用绑定公开的 `IUIAutomationElement_Vtbl::GetCurrentPattern`，输出指针预置 NULL，保存 HRESULT 与指针后才转换为接口并正确释放引用；这属于精确观测的可选实现，不是本轮盘点的前提。[固定 vtable 定义](https://github.com/microsoft/windows-rs/blob/32c3144490c016fe496a0aed769bce60987a2e9d/crates/libs/windows/src/Windows/Win32/UI/Accessibility/mod.rs#L10117)

## 值得补读的元数据与禁止混入的动作

取得 Legacy 接口后可读取下表的九个 getter；其中 State、Value、KeyboardShortcut、ChildId、Name、Description、Help 是已接受 inventory 没有读到的项目。[Legacy 客户端接口](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nn-uiautomationclient-iuiautomationlegacyiaccessiblepattern)

| getter | 有用事实及解释边界 |
| --- | --- |
| `CurrentState` | 保存原始 `u32` 位掩码及已知 flag 名称，包括 focused/focusable、readonly、invisible/offscreen、expanded/collapsed；它是 MSAA 报告状态，不能当可见编辑态的充分条件。[状态常量](https://learn.microsoft.com/en-us/windows/win32/winauto/object-state-constants) |
| `CurrentValue` | 保存 MSAA 当前字符串；与 UIA Value 分开比较，值的存在不代表已提交文件夹。 |
| `CurrentDefaultAction` | 本地化动作说明。MSAA 文档解释其描述动作方式，不描述最终业务结果；空值或“打开/按下”不能据此推出提交机制。[MSAA getter 合约](https://learn.microsoft.com/en-us/windows/win32/api/oleacc/nf-oleacc-iaccessible-get_accdefaultaction) |
| `CurrentKeyboardShortcut` | MSAA 快捷键信息，只记录，不发送按键。 |
| `CurrentChildId` | MSAA 子对象标识；`CHILDID_SELF=0` 表示对象本身；不把它当 HWND 或全局 UIA 身份。[Legacy 模式成员](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementinglegacyiaccessible) |
| `CurrentRole` | 保留数字 role，供同一目标、同一采样状态比较。 |
| `CurrentName`、`CurrentDescription`、`CurrentHelp` | 保留名称、描述、帮助文字；它们是线索，不是调用动作的授权或准确效果保证。 |

所有 getter 分别记录成功/错误；成功返回空字符串也要保存，不能再用 `.ok()` 将错误与空值合并。Legacy 的底层 MSAA 来源、UIA 原生 provider 与 proxy 存在边界，因此 Legacy 缺失不能否定其它 UIA 能力。[访问 MSAA servers](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-accessingmsaaservers)

额外读取 UIA `AccessKey`（30007，访问键/助记键，access key）与 `AcceleratorKey`（30006，快捷键，accelerator key），用 `GetCurrentPropertyValueEx(..., true)` 保留不支持与空串差别；两者都是 `VT_BSTR` 元数据，不能把它们翻译成模拟输入方案。同步保存 UIA 的 enabled/offscreen/focusable/has-focus、bounding rectangle 与原生 HWND 可见性，以区分报告来源。[UIA 元素属性](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-automation-element-propids)

若已有 Value/ExpandCollapse 接口，可读 `CurrentValue`、`CurrentIsReadOnly`、`CurrentExpandCollapseState`；`IsReadOnly=false` 仅报告该 value 可修改，不表示控件已显示、文本可提交或能到达 B。[Value 接口](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nn-uiautomationclient-iuiautomationvaluepattern)、[ExpandCollapse 接口](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nn-uiautomationclient-iuiautomationexpandcollapsepattern)

只读白名单到此为止，不遍历各模式的全部方法。`Invoke` 执行动作，`SetFocus` 改变键盘焦点，Value/Legacy `SetValue` 修改值，`Expand/Collapse` 改变显示状态，Legacy `Select/DoDefaultAction` 执行选择/默认动作；它们均不能混入读取过程。[Invoke](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationinvokepattern-invoke)、[SetFocus](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationelement-setfocus)、[Value](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nn-uiautomationclient-iuiautomationvaluepattern)、[ExpandCollapse](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nn-uiautomationclient-iuiautomationexpandcollapsepattern)、[Legacy](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nn-uiautomationclient-iuiautomationlegacyiaccessiblepattern)

## 建议交给人选择的下一次调查

以下是待选择的原型规格，本研究没有执行，也不代替 #79 的人类确认。

1. **一次 controlled Open、初始文件夹 A、零 provider 动作。** 维持原有 filename/filter，不手动预置 editor、不主动设置前台、不停止服务、不进行自动重跑。外部客户端只采用 B（UIA 地址栏）策略，不使用模拟输入、剪贴板、原生 helper 或 fallback。自有 fixture 的 native `IFileDialog` 仅作初始设置和独立只读真值，不成为外部客户端取得的另一导航后端。[已接受 fixture 的边界](https://github.com/inaku-Gyan/PathWrap/blob/a1daeeec7db641336adac80934053b3765ee1247/docs/prototypes/uia-current-folder-breadcrumb/README.md)
2. **固定唯一目标与窄范围。** 绑定原 dialog HWND/PID 和 Address Band Root；只遍历其 Raw 子树（包含 Breadcrumb Parent/toolbar），另从该地址带限定的 HWND 后代中寻找已知 ComboBoxEx32/ComboBox/Edit，再分别 `ElementFromHandle` 读取其 Raw 子树；不因隐藏而跳过，不扩展至桌面或其它应用。来源链、父子关系、重复元素身份均记录，目标歧义立即终止。Raw view 与基点 scope 是官方支持的树读取方式；`ElementFromHandle` 是按 HWND 取得元素，但文档不保证任意隐藏控件都能提供完整 provider。本机隐藏控件可读取的依据来自已接受原型，仍须逐项记录本次结果。[树与 scope](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-treeoverview)、[ElementFromHandle](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation-elementfromhandle)、[已接受发现代码](https://github.com/inaku-Gyan/PathWrap/blob/a1daeeec7db641336adac80934053b3765ee1247/src/bin/uia_current_folder_breadcrumb_prototype.rs#L526)
3. **一次有预算的读取 pass。** 先预登记目录及上面的 getter 白名单。例如最多 64 个唯一元素、深度 16、4096 次 provider 调用、软预算 10 秒，沿用独立 MTA worker 的 15 秒 supervisor 保底；数值是建议的实验预算，不是微软性能保证。计入树行走、属性、绑定和 getter 调用；若不足完成就输出 `incomplete` 和未读清单，不放宽范围或重试。官方建议 UIA 工作在无窗口的独立 MTA 线程；读取会跨进程且树是动态的，不能把“只读”理解成原子快照或保证不会阻塞。[线程建议](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-threading)、[跨进程读取](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationelement-getcurrentpattern)、[动态树](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-treeoverview)
4. **输出能改变下一次决策的事实。** 比较 visible breadcrumb 与 hidden editor 的 35 项可用性、Legacy 状态/值/默认动作/快捷键/子标识及 UIA 访问键信息，保留来源、空值、错误、遗漏与时间。若 folder/focus/visible 状态自行漂移，就标注漂移，不能称为稳定初始 A 的同一状态；丢失目标则停止。即便全部完成，也只说明该次状态中的可发现能力，不证明编辑进入/提交/到达 B。[模式读取的时点语义](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationelement-getcurrentpattern)

如将来希望补测已经观察过的下拉状态，那是**第二个需要人选择的状态实验**；不能在首轮只读 pass 内偷偷调用 ComboBox Invoke、Expand、Select 或默认动作来预置它，也不能从现有截图推出其全部模式。当前不需要该状态即可补齐初始 A 和隐藏编辑器的遗漏信息。

## 本次验证与剩余未知

本次验证仅为文档与源码核对：核对固定锁文件/本地绑定、35 对目录常量、Legacy getter 和 Rust 空接口转换；提交前检查 diff 与目录映射。只新增本笔记，不修改产品、既有原型或用户的 GLOSSARY.md；无 Rust 代码变更，不运行 Rust 测试或 GUI。

仍未知：本机目标每项新模式/属性的实际结果、查询耗时、可见编辑进入方法、任意 B 提交与可靠外部到达验证。官方接口的存在不解决这些未知。#80 的事实前提可据此解决；#79 仍须由人选择是否实施上述只读调查，不能把本研究当作任何 GUI 运行授权。
