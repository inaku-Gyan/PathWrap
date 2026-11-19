# 便携 PathWarp 的当前用户自启登记与状态研究

研究日期：2026-11-19。对应[研究票 #96](https://github.com/inaku-Gyan/PathWrap/issues/96)，所属[设置地图 #95](https://github.com/inaku-Gyan/PathWrap/issues/95)。本记录提供平台事实、实现候选与验收边界；[人机决策票 #98](https://github.com/inaku-Gyan/PathWrap/issues/98)仍须确认交互、失败及恢复策略，产品实现仍由[设置界面 #30](https://github.com/inaku-Gyan/PathWrap/issues/30)交付。

源码基线为 `01fa41ba1ec23492be544fdaf7e129cf1b3a94c2`。仅阅读官方文档、该基线与实际依赖源码；未运行产品、写入自启登记、读取用户真实自启项或执行桌面测试。文中的“建议／推断”不是已通过系统验收的结果。

## 结论与首版候选

1. **登记存在（registration present）与实际允许登录启动（effective startup permission）必须分开。** 当前用户 `Run` 值是有公开文档的持续登录启动登记；Windows 设置与任务管理器另有用户可控制的启停状态。前者写入并读回成功，只能确认登记内容，不能据此承诺后者已允许启动，更不能承诺下次登录一定启动。[Run 文档][run]；[Windows 自启设置][startup-settings]
2. **便携、未打包首版的最小候选是一个固定名称的 HKCU Run 值**，把所选程序绝对路径与配置目录绝对路径共同写入命令。其整条命令有公开的 260 字符上限；不能因为注册表字符串可容纳更长数据就接受超限登记。若该限制不可接受，当前用户 Startup 中的固定 `.lnk` 是另一个候选，但其读取、路径长度和移动追踪也有边界。[Run 文档][run]；[Shell 链接接口][shell-link]；[GetPath 限制][link-getpath]
3. **本次未找到可作为兼容性契约的、针对任意传统 Run／Startup 链接的公开“Windows 已禁用／已允许”查询接口。** 查阅范围是本记录链接的 Run、注册表、Shell 链接、自启设置和 StartupTask 官方资料。`StartupApproved` 的二进制布局不在这些 API 合同内；不把反向推测的字节值或“没有该值”映射成已启用。这是本次研究的证据边界，不是对所有未来 Windows API 的不存在证明。[Run 文档][run]；[注册表读取 API][reg-get]；[Shell 链接接口][shell-link]；[StartupTask 适用范围][startup-task]
4. **现有 `ConfigStore` 不能向设置窗口返回逐项保存结果。** `save` 返回 `()`，首次写入失败后只记日志并切换为仅内存（memory-only），本进程后续保存直接跳过；读错误、解析回退与实际落盘也没有结构化结果。设置窗口要如实显示“已持久化／未保存／仅本次生效”，需在实现票中补齐结果报告；是否回退主题或支持本次重试由 #98 决定。[配置存储源码][p-config]；[主题更新调用][p-app]

| 候选机制 | 无提权的当前用户范围与读取／关闭 | 可确认与限制 |
| --- | --- | --- |
| HKCU `Software\Microsoft\Windows\CurrentVersion\Run` 固定命名值 | 在当前用户 ACL 允许时，用普通注册表 API 查询、设置或删除该一个值；无需建立系统级任务。权限不足仍可失败，不能将“当前用户”写成“保证可写”。[权限][reg-rights]；[设置][reg-set]；[删除][reg-delete] | 可读回类型和命令；没有工作目录字段；整条命令 ≤260 字符；启动时序和立即执行无保证。[Run][run] |
| `FOLDERID_Startup` 下固定 `.lnk` | 该 Known Folder 为 `PERUSER`；`SHGetKnownFolderPath(..., hToken=NULL)` 获取当前用户位置。读写本用户链接仍受文件 ACL 约束；删除本链接可移除该登记。[Known Folder][known-folder]；[定位 API][known-folder-path]；[系统支持说明][startup-settings] | `IShellLinkW` 分开保存／读取程序路径、参数和工作目录；链接存在也不能推导系统允许态。`GetPath` 公开返回上限为 `MAX_PATH`，不能宣传普遍解决长路径。[ShellLink][shell-link]；[GetPath][link-getpath] |
| 当前用户登录触发的计划任务（Task Scheduler） | 普通低权限进程可注册 `TASK_RUNLEVEL_LUA`，不能注册最高权限／系统账号任务；`TASK_LOGON_INTERACTIVE_TOKEN` 可沿用连接用户身份。[安全上下文][task-security] | `RegisteredTask.Enabled` 有公开读写合同，但表示该计划任务的启停，不是传统 Startup apps 的禁用态。[Enabled][task-enabled] 它增加另一种系统登记对象；本研究不建议为避开 Startup apps 的用户控制而自动改用它。是否另选该机制须重新明确产品范围。 |
| 打包应用 `Windows.ApplicationModel.StartupTask` | 需要应用包和 manifest 中的 startup 扩展；不是给任意便携 EXE／Run 值附加状态查询的 API。[StartupTask][startup-task] | 有 `DisabledByUser`／`DisabledByPolicy` 等状态，且请求启用不能覆盖用户经任务管理器作出的禁用选择。不能把这些状态直接套用到当前未打包 PathWarp。[StartupTask][startup-task] |

**研究建议：** #98 可以在“HKCU Run + 明确展示登记状态、系统允许态未知”和“当前用户固定链接 + 相同状态边界”之间选择。若要求设置窗口准确同步 Windows 外部的允许／禁用，当前证据不足以为普通便携机制提供这一承诺；应保留“无法验证”并让用户去系统设置确认，或重新研究打包方案，不应把 `StartupApproved` 猜测伪装成已验证事实。以上是依据上表能力的工程推断。

## 当前项目与实际依赖

### 存储目录语义

`main` 先用 `args_os().skip(1)` 解析参数，再用 `current_exe()` 定位程序，将选择交给 `ConfigStore::open`。公开 CLI 只接受一次 `--storage-dir <PATH>`；缺失值、空值、重复该参数及其他参数都报错退出。默认目录是 EXE 同级的 `pathwrap-store`，文件名为 `config.json`。普通相对目录通过程序目录 `.join(...)` 计算，与进程启动工作目录无关；绝对输入直接保留。[启动源码][p-main]；[参数与目录计算][p-config]

Windows 有一个必须保留的边界：`C:store` 不是 `C:\store`。Rust Windows 的 `Path::join` 遇到有盘符前缀但无根的输入会返回该输入，故当前函数可能仍输出驱动器相对路径；`\store` 则保留程序盘符并替换根以下部分。现有“相对路径在 EXE 旁”的测试只覆盖普通相对路径，没有覆盖这两种 Windows 形式。[配置源码及测试][p-config]；[Rust PathBuf／join][rust-path]；[Windows 路径命名][path-naming]

**推断／建议：** 启用时冻结实际选择的完整目录，不复制原始参数字符串，不依赖下一次 Shell 的工作目录。对于当前解析结果仍非全限定的目录，应在启动／存储打开时确定并保留其有效绝对路径，或拒绝无法确认的输入；事后用已经变化的工作目录重新解释可能指向另一目录。`std::path::absolute` 在 Windows 普通路径上当前等价于 `GetFullPathNameW`，不访问文件系统，也不证明目录存在或可写；可作冻结工具，不能作可用性证明。此边界由 #98 明确首版支持程度。[目录计算源码][p-config]；[Rust absolute][rust-absolute]

### 可直接复用的绑定及缺口

基线锁定 `windows`／`windows-core` **0.62.2**、Rust **1.100.0**；`Cargo.toml` 已声明 `Win32_System_Registry`、`Win32_UI_Shell`、`Win32_System_Com` 和文件系统等功能。实际下载的 `windows-0.62.2/.cargo_vcs_info.json` 指向 Microsoft windows-rs 提交 `32c3144490c016fe496a0aed769bce60987a2e9d`，以下绑定按该提交核对，而非按最新 SDK 猜测。[项目依赖][p-cargo]；[锁文件][p-lock]；[工具链][p-toolchain]；[依赖源快照][w-registry]

- `RegOpenKeyExW`、`RegGetValueW`、`RegQueryValueExW`、`RegSetValueExW`、`RegDeleteValueW` 返回 `WIN32_ERROR`；不能忽略返回状态或假设它们都返回 Rust `Result<()>`。`RegSetValueExW` 的数据参数为字节切片，UTF-16 的终止 NUL 及字节编码由调用方提供。[实际注册表绑定][w-registry]；[RegSetValueExW][reg-set]
- `RegCreateKeyExW` 在该版本另受 `Win32_Security` feature 门控；项目基线未直接声明该 feature。若选择“Run key 缺失时创建”，实现时应直接声明所用 feature，不能依赖偶然的传递 feature 合并。[绑定门控][w-registry]；[项目依赖][p-cargo]
- 同版有 `SHGetKnownFolderPath`、`FOLDERID_Startup` 和 `IShellLinkW` 的读写方法，现有 Shell／COM 依赖可作为链接候选的基础；这不表示产品已有链接持久化、登记管理或系统允许态实现。[实际 Shell 绑定][w-shell]；[基线启动源码][p-main]

## 单一用户登记、归属与观测模型

地图已确认“同一用户一个 PathWarp 自启项，以最后一次明确启用的程序副本和配置目录为准；其他副本启动不改写”。[地图 Notes][map] **实现推断：** 所有副本使用同一个、稳定且足够独特的命名位置，例如 HKCU Run 的 `inaku-Gyan.PathWarp` 值，或 Startup 下的同名固定链接；名称不含版本号、EXE 路径哈希或配置目录。首版只管理所选这一机制的这个位置，不同时创建 Run 值和链接。

可用下面的观测数据代替一个 `bool auto_start`。它是读回结果的模型，不是新增持久化状态源：

| 观测维度 | 可表达的结果 | 解释 |
| --- | --- | --- |
| 登记读取 | `Absent` / `Present(raw type + data)` / `Unreadable(error)` | `ERROR_FILE_NOT_FOUND` 与拒绝访问、数据变化、缓冲区失败必须分开。[RegQueryValueExW][reg-query] |
| 登记内容 | `Recognized(exe, storage)` / `Unrecognized(reason)` | 仅接受产品定义的命令语法／链接字段。未知类型、额外参数、相对目标、残缺内容不能强行猜测。此为防止误改的建议。 |
| 与当前进程选择的关系 | `ThisTarget` / `OtherTarget` / `Uncertain` | 两个路径组成目标元组；EXE 相同但配置目录不同也不等于本次选择。它表示登记目标关系，不证明文件作者身份。 |
| 当前可达性 | `TargetPresent` / `TargetMissing` / `Unverifiable` | 文件现在可见、确定不存在、访问失败要分开；不能将可移动磁盘暂离等同于永久无效。系统支持文档也列举可移动盘、网络盘及路径不存在导致的缺失项。[系统缺失项说明][startup-settings] |
| Windows 允许状态 | 对传统 Run／链接为 `Unknown` | 上述公开查询没有在本研究范围内提供可靠的允许态；即使命令恰好匹配，也保留该未知维度。 |
| 上次操作结果 | `NotAttempted` / `VerifiedRegistration` / `VerifiedAbsent` / `Failed` / `Unverified` / `Conflict` | 成功级别只到本次读回能证实的事实；不记录成下次登录已验收。此为结果报告建议。 |

“其他副本”可以先按已识别命令的不同完整路径或不同配置目录报告；路径大小写、短文件名、联接、符号链接等别名不应由简单字符串小写就认定为同一物理副本。`current_exe` 返回完整路径，但重命名、链接和查询失败都有公开边界；路径文本也不能验证文件仍是原来的二进制。[Rust current_exe][rust-current-exe]；[Windows 命名与链接][path-naming] **建议：** 对产品自己刚写入的规范形式精确读回比较；对其他形式无法证明等价时保持 `Uncertain`，不要为了“修复”而静默改写。

**归属建议：** 限定命名位置与已识别语法是一项操作防护，不是密码学身份保证。任何同用户程序都可能伪造同名登记或替换目标文件。遇到同名未知内容，应显示冲突并保留原始数据，交由 #98 决定显式接管入口；不要仅凭值名或 `PathWarp.exe` 文件名删除。关闭已识别的其他副本登记是否算“关闭全局项”，也需在 #98 的动作文案中说清。不要枚举并删除所有含 `PathWarp` 字样的 Run 值、链接或计划任务。

“一个自启项”的可兑现范围是 **PathWarp 自己管理的位置**。用户手工放置的另一链接、HKLM、其他任务或第三方登记可能仍启动它；既要避免触碰无关应用，就不能承诺清除全系统所有入口。只在选定位置写入固定值，不手工拼 `Wow6432Node`；64 位与 32 位 registry view 有专门系统规则，项目当前目标为 x86_64，未来架构迁移若改变登记位置需另作迁移研究。[注册表视图规则][registry-views]；[工具链目标][p-toolchain]

## 可执行文件、参数引用与工作目录

启用候选应序列化这一逻辑命令，其中两个路径均为启用时确认的绝对路径：

```text
"D:\Portable Apps\PathWarp\PathWarp.exe" --storage-dir "E:\个人设置\PathWarp"
```

即使本次没有传 `--storage-dir`，也可把计算后的默认目录显式写入；这样登记直接表达“所选程序＋所选配置”，无需另存一个 JSON 自启开关。未来从其他目录启动 B 副本，仅观察这个 A 的登记，不会替换为 B。[地图归属约束][map]；[当前目录解析][p-config] 这是建议的表示方式，不是当前产品已有行为。

- 程序路径必须独立完整引用；有空格却未引用的程序路径会在进程创建时产生歧义。不要借助 `cmd /c`、PowerShell、脚本或文件关联启动 EXE，因为它们增加另一套解析规则。[CreateProcessW 安全说明][create-process]
- 参数编码以 **Rust 目标进程实际使用的 Windows argv 规则**为准。Rust 1.100.0 自己解析 `GetCommandLineW`，明确没有使用 Shell 的命令行解析函数；不能只用 `CommandLineToArgvW` 的输出当成等价性证明。[实际 Rust 参数源码][rust-args-source]
- 对常规参数，空格／tab 需引用；引用内尾随的反斜杠在闭合引号前必须加倍。例如目录 `E:\settings\` 的参数编码为 `"E:\settings\\"`。程序名 `argv[0]` 又有单独规则，不应无差别套用普通参数编码。使用 UTF-16／`OsString`，不要通过 `display()` 或有损字符串回写路径。[Microsoft 参数规则][argv-rules]；[Rust 编码与解析源码][rust-args-source]
- Run 登记没有单独指定工作目录的合同；不要假设它等于 EXE 所在目录，更不要依赖它找配置。链接可通过 `SetWorkingDirectory` 显式设置程序目录，同时仍传绝对 `--storage-dir`；这可减少环境差异，但不能解决权限或目标离线。[Run][run]；[ShellLink][shell-link]；[当前配置计算][p-config]
- Run 的 260 字符是整条登记命令的限制，不是仅限制 EXE。**建议**按完整序列化结果（含引号、空格与参数）作保守 UTF-16 长度校验，拒绝截断；官方此处没有细化非 BMP 字符与终止符的计数约定，边界应以保守预算和后续系统验收确认，不宣称该处长路径支持。[Run][run]
- 链接 `SetArguments` 的 Unicode 参数字符串没有该方法层面的长度上限，但 `GetPath` 有 `MAX_PATH` 合同、可能返回 `S_FALSE` 与空路径；需要同时验证返回字段，不能仅看绑定 `Result` 是否 `Ok`。不能由一个方法的宽松长度推出 Shell 启动全链路无上限。[SetArguments][link-arguments]；[GetPath][link-getpath]；[实际绑定][w-shell]

## 移动、升级、外部修改与失败

### 可以确认到哪一层

| 情形 | 可以确认 | 不能据此承诺／建议动作 |
| --- | --- | --- |
| A 位置同路径替换升级 | 登记仍是相同命令、同一配置目录；基线 README 要求替换 EXE 时保留设置目录。[README][p-readme] | 路径相同不证明新文件兼容、可运行；本次不做启动测试。 |
| 程序或配置目录移动 | Run 原命令仍是原路径；读回可发现与本次选择不同。Shell 链接可能在 Shell 使用时自动解析并追踪目标，应用也可主动调用 `Resolve`。[Run][run]；[Shell 链接解析][links-overview]；[Resolve][link-resolve] | 不应宣传自动跟随所有移动。普通状态读取不应主动 Resolve／保存更改，但这也不能保证 Shell 登录启动时绝不追踪。若契约严格锁定原 EXE 路径，Run 更直接；链接的追踪控制需补充研究与验收。是否允许追踪或要求重新启用，由 #98 决定。 |
| B 副本启动 | 可读取仍指向 A／A 配置的登记。[地图约束][map] | B 的启动不构成接管意图；读状态不可顺便“修复”为 B。 |
| Windows 设置／任务管理器禁用 | 官方保证用户可阻止其登录启动。[系统设置][startup-settings] | 即使登记读回存在，也不能显示“系统已允许”。不写 `StartupApproved` 来绕过用户选择；研究没有验证“重写 Run 必然重新允许”。 |
| 外部修改／删除／权限变化 | 重新读取时可得到当时的原始值、缺失或读取错误。[RegQueryValueExW][reg-query] | 缓存只表示旧快照；建议打开设置、执行操作后及显式刷新时重新读，冲突时停止使用旧归属推断。 |
| 路径存在、登记成功 | 本次登记字段存在且读回一致。[RegSetValueExW][reg-set]；[RegGetValueW][reg-get] | 不证明下次登录可用、磁盘在线、配置可读、依赖可加载或应用初始化成功；即使 CreateProcess 返回成功也不等待完成初始化。[CreateProcessW][create-process] |

### 保留原登记的能力与限制

以下是基于普通注册表 API 合同的**建议操作序列**，不是已选择的 UI 失败策略：

1. 先只读现值，保留存在性、类型和原始字节；验证新程序／配置路径和完整命令后，再进入写入阶段。需要原样快照时用 `RegQueryValueExW`，并自行防护未终止字符串；`RegGetValueW` 为字符串补终止符或展开环境变量的便利不应改变原始备份。两阶段分配读取可能遇到 `ERROR_MORE_DATA`，应有有界重试；不能把读失败当“不存在”。[RegQueryValueExW][reg-query]；[RegGetValueW][reg-get]
2. 启用时直接设置同一个值，避免“先删旧值，再写新值”的空窗；关闭仅删这个已确认可管理的命名值，绝不删整个共享 Run key。[设置][reg-set]；[删除][reg-delete]
3. 成功返回后再读回。匹配只能报告“登记写入并读回”；关闭后读到缺失只能报告“该登记已移除”。写入／删除返回错误或读回失败时，实际状态仍需读取证据；不擅自显示恢复完成。[设置][reg-set]；[读取][reg-get]
4. 若尝试补偿回滚（compensating rollback），先重新确认当前位置仍是本次写入的值，再尝试恢复旧类型／旧字节，并再次读回。普通查询＋设置并非条件更新／比较交换（compare-and-swap）接口，重读与恢复间仍有竞态；文件、权限及另一副本变化也可能使补偿失败。因此“任何失败都保留原登记”无法由这一序列无条件保证。[API 参数与作用范围][reg-query]；[RegSetValueExW][reg-set]

多个 PathWarp 进程可通过同用户互斥串行化本产品的明确启停操作，减少相互覆盖，但不能锁住 Windows 设置或第三方编辑器；无修改意图的启动不应拿到锁后写回旧快照。这是并发设计建议。未知同名值是否允许显式覆盖、原登记未确认恢复时显示什么、是否支持重试，是 #98 的产品选择。

若选链接，先在同目录的唯一临时文件构造完整链接、验证字段，再替换固定链接，可避免先删除旧链接；`IPersistFile::Save` 可把链接对象保存到指定文件，但应重新载入核对而非仅凭保存调用成功。对象保存、文件替换、Shell 追踪和外部编辑仍不是跨进程事务。不要把“先写临时文件”推广成“任意故障绝不丢失原状态”。文件替换 API 同样受 ACL 与失败返回限制。[Shell 链接持久化][links-overview]；[IPersistFile::Save][persist-save]；[MoveFileExW][move-file]

### 原配置与持久化报告缺口

当前 `write_config_atomic` 把新 JSON 写入固定 `config.json.tmp`，`write_all` 后 `sync_all`，再以 `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)` 替换正式文件，错误时尝试删临时文件。因此序列化、临时文件写入等替换前失败不会主动截断正式文件；但多副本共用固定临时名会竞争，也没有配置版本比较。它不能作为多进程隔离或文件＋注册表统一事务的保证。[配置写入源码][p-config]；[MoveFileExW][move-file]

具体缺口是：[配置存储源码][p-config]；[现有主题调用][p-app]

- `ConfigStore::open` 只返回 `(Store, AppConfig)`：目录创建／读取失败转仅内存；JSON 解析失败则回退默认值但仍允许后续保存，没有结构化警告交给 UI。`AppConfig` 还使用 `deny_unknown_fields`，故未知字段也触发该解析回退，后续保存可能覆盖原文件内容。
- `ConfigStore::save` 无成功／失败返回；`is_memory_only` 仅给模式位，不能说明刚才是成功保存、已跳过、写入报错还是启动时已不可用，也不能提供错误原因。没有公开读取所选配置路径的接口。
- 当前主题先在内存和 UI 生效再尝试保存。失败后保持新主题且本次会话不再尝试落盘；设置窗口不能在此基础上给“已保存”提示。
- 固定临时名、缺少多进程协调和丢失更新（lost update）检测，意味着两个副本共用同一配置目录时既可能相互覆盖，也可能使保存失败；研究没有模拟或验证其具体时序。

**建议：** 实现应向 UI 报告所选路径、加载诊断及每次保存的 `Saved / SkippedMemoryOnly / Failed(error)` 等结果，并区分“本次内存已应用”和“磁盘已持久化”；若 #98 选择重试，需明确退出仅内存模式的流程。不要同时向 JSON 写一个声称代表系统事实的自启布尔值。主题文件与 Windows 登记是两个独立副作用：先做哪一个都可能部分成功，普通注册表操作与当前文件写入没有共同提交／回滚机制。#98 应决定是否独立提交、是否补偿以及部分结果的提示，不能承诺全有或全无。[当前配置实现][p-config]；[注册表设置合同][reg-set]

## 后续实现的验收分层

以下均是**建议验收点，尚未运行**；该研究分支只含文档。

### 纯模型与隔离测试

| 范围 | 需覆盖的行为 |
| --- | --- |
| 观测归约 | 缺失、已识别本目标、已识别其他 EXE／同 EXE 其他配置、未知类型／语法、读拒绝、失效路径、路径无法验证；传统机制所有匹配路径仍保留系统允许态未知。 |
| 命令表示 | 空格、中文、非 BMP 字符、UNC、盘符根目录、尾反斜杠、普通相对目录、`C:store`／`\store`、多余参数及超长命令；解析后必须精确保留 EXE 和一个 `--storage-dir` 值。以实际 Rust argv 语义和项目 parser 为判据，不能仅验证字符串外观。 |
| 单一登记 | A 明确启用后 B 只读启动不写；B 明确启用替换同一个管理位置；同 EXE 不同配置有可见差异；其他值／HKLM／其他链接完全不变；未知同名内容保持冲突。 |
| 故障注入 | 读取、打开、设置、删除、读回、补偿分别失败；操作返回成功但读回不同／无法读取；外部修改插入各阶段；报告不得超过证据，不能无条件宣称旧登记已恢复。 |
| 配置保存 | 保存成功、创建／读取失败、损坏／未知字段、临时写失败、替换失败、已处仅内存时跳过；磁盘旧配置是否保留分别断言，UI 能区分已应用与已保存；本次可重试与否按 #98 的最终契约。 |
| 多进程 | 同一配置目录与固定临时名的竞争、丢失更新、互斥覆盖范围与外部修改；模型应注入交错，不能用一次顺序 round-trip 测试代表并发安全。 |

纯模型可用内存后端模拟注册表／文件返回值；原生注册表适配器若需验证，可使用测试专属随机命名的 HKCU 非启动子键，链接使用临时目录而非真实 Startup，且只清理该测试创建的资源。它们证明序列化和适配器行为，不证明 Windows 登录时的 Shell 执行。以上属于后续实现测试建议；本票不创建这些资源。

### 仍需另行授权的真实 Windows 验证

1. 在产品支持的 Windows 10／11 构建、普通用户权限下，验证所选真实机制可登记、在系统界面可识别并于下次登录启动；记录版本／构建、目标盘类型、路径与实际配置目录。研究未执行这些操作。
2. 经 Windows 设置及任务管理器各禁用／恢复一次，观察应用界面的登记状态与系统允许态表述；确保刷新不重新启用、不写私有 `StartupApproved`。传统机制即使无法检测禁用，也必须如实保留“无法验证”。
3. 覆盖便携移动、同路径升级、第二副本显式接管与只读启动、外部编辑登记、可移动／网络目标离线；若采用链接，记录 Shell 追踪行为，不能用 Run 的结果替代链接验收。
4. 验证长命令拒绝、Unicode 路径、工作目录不同、配置不可写等条件下，实际启动仍选中预期配置，或显示明确的未验证／失败；不以登记读回替代运行结果。

这些系统行为会改变真实桌面和启动登记，必须留到实现验收阶段按授权范围进行。#98 可以基于本记录先完成人机契约，不需把尚未进行的系统实验冒充研究事实。

## 交给 #98 的明确选择

- 选择 Run 或固定链接，以及长度／路径／移动支持边界；若保留传统便携机制，如何显示“已登记，但 Windows 允许状态无法验证”。
- 已识别的其他副本、同名未知登记、失效目标分别允许哪些动作，是否显式接管；关闭的语义是删除产品管理的全局项还是只处理本目标。
- 主题即时应用与持久化失败后保留／回退的表现；仅内存状态、本次重试和错误细节的契约。
- 配置与登记部分成功时独立结果如何展示，是否尝试补偿，补偿／读回失败如何保留未知状态；不承诺超出已验证事实的“成功启用”。

[map]: https://github.com/inaku-Gyan/PathWrap/issues/95
[p-config]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/src/config.rs
[p-main]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/src/main.rs#L30
[p-app]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/src/app.rs#L114
[p-cargo]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/Cargo.toml
[p-lock]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/Cargo.lock#L3447
[p-toolchain]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/rust-toolchain.toml
[p-readme]: https://github.com/inaku-Gyan/PathWrap/blob/01fa41ba1ec23492be544fdaf7e129cf1b3a94c2/README.md#L19
[w-registry]: https://github.com/microsoft/windows-rs/blob/32c3144490c016fe496a0aed769bce60987a2e9d/crates/libs/windows/src/Windows/Win32/System/Registry/mod.rs
[w-shell]: https://github.com/microsoft/windows-rs/blob/32c3144490c016fe496a0aed769bce60987a2e9d/crates/libs/windows/src/Windows/Win32/UI/Shell/mod.rs#L39950
[run]: https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys
[startup-settings]: https://support.microsoft.com/en-us/windows/experience/startup-boot/configure-startup-applications-in-windows
[reg-rights]: https://learn.microsoft.com/en-us/windows/win32/sysinfo/registry-key-security-and-access-rights
[reg-set]: https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regsetvalueexw
[reg-delete]: https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regdeletevaluew
[reg-get]: https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-reggetvaluew
[reg-query]: https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regqueryvalueexw
[registry-views]: https://learn.microsoft.com/en-us/windows/win32/sysinfo/32-bit-and-64-bit-application-data-in-the-registry
[shell-link]: https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ishelllinkw
[links-overview]: https://learn.microsoft.com/en-us/windows/win32/shell/links
[persist-save]: https://learn.microsoft.com/en-us/windows/win32/api/objidl/nf-objidl-ipersistfile-save
[known-folder]: https://learn.microsoft.com/en-us/windows/win32/shell/knownfolderid#folderid_startup
[known-folder-path]: https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shgetknownfolderpath
[link-getpath]: https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishelllinkw-getpath
[link-arguments]: https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishelllinkw-setarguments
[link-resolve]: https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishelllinkw-resolve
[task-security]: https://learn.microsoft.com/en-us/windows/win32/taskschd/security-contexts-for-running-tasks
[task-enabled]: https://learn.microsoft.com/en-us/windows/win32/taskschd/registeredtask-enabled
[startup-task]: https://learn.microsoft.com/en-us/uwp/api/windows.applicationmodel.startuptask?view=winrt-26100
[create-process]: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw
[argv-rules]: https://learn.microsoft.com/en-us/cpp/c-language/parsing-c-command-line-arguments?view=msvc-170
[rust-args-source]: https://github.com/rust-lang/rust/blob/0ef1bc386059596dada8fb65b50425aab2ea637a/library/std/src/sys/args/windows.rs
[rust-path]: https://doc.rust-lang.org/std/path/struct.PathBuf.html#method.join
[rust-absolute]: https://doc.rust-lang.org/std/path/fn.absolute.html
[rust-current-exe]: https://doc.rust-lang.org/std/env/fn.current_exe.html
[path-naming]: https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file
[move-file]: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw
