# 目标：euphoria 中文启动器

中文版可执行文件是主要兼容性目标。商业文件保留在已忽略的 `tests/targets/` 目录下。

## 本地指纹

- 映像：`tests/targets/euphoria/inspect/euphoriaCN.exe`
- SHA-256：`ea36a153a20453fc9ff4254233050528320c5f83a6a06c63c3de052b42940072`
- 格式：PE32 GUI，Intel 80386
- 首选基址：`0x00400000`
- 入口点：`0x00402798`
- 映像大小：`0x00744000`
- 静态导入：49 个，全部来自 Kernel32

## 已观察路径

启动器初始化其 CRT 与中日文 ANSI 转换表，打开自身可执行文件，读取大型附加数据，并在来宾堆上解压出可执行代码。发布版 Runner 大约需要 2 亿条解释指令才能越过该解压阶段。

解压后的加载器将 `INT3` 用作未解析导出的失败路径。Runtime 现在通过真实的 32 位来宾 SEH 链分发断点异常，而不是将其视为不支持的指令。栈诊断按顺序识别出了缺失的导出。

已完成的加载器表层：

- 为已建模的宿主 DLL 实现 `LoadLibraryA/W`。
- 为每个已注册 API 模块自动生成合成模块映像。
- 可加载的合成 `ntdll.dll`。
- 已观察的 NTDLL 导出普查现还覆盖加载器的注册表、进程、线程、文件、对象以及虚拟内存探测。最新解析到边界是 `NtDuplicateObject`；未解析名称仍保留为显式解析存根，因此真实调用会在精确的 API 边界处失败。

已实现第一批目标相关的 NTDLL 语义：

- 针对当前进程的 `NtAllocateVirtualMemory`。
- 针对当前进程来宾内存的 `NtReadVirtualMemory` 与 `NtWriteVirtualMemory`。
- 使用 32 位 `UNICODE_STRING` 布局的 `RtlInitUnicodeString`。
- 按文档约定为单线程空操作的 `RtlAcquirePebLock` 与 `RtlReleasePebLock`。

导出普查现已完成，启动器在 `0x70100000` 附近映射了一个解压后的子映像。其导入加载器读取合成 DLL 导出映像，并已越过 Kernel32 和 NTDLL 导入，进入普通来宾代码。

已执行的兼容性路径现包括：

- 已建模的 `advapi32.dll`，包含进程令牌、`TokenUser`、SID 字符串，以及跨 DLL 分配/释放语义。
- 分离的虚拟地址预留与页面提交，以及 `VirtualProtect` 权限转换与可读的宿主跳板 facade。
- Windows 完整/短/长路径转换、文件时间查询，以及带有自有 `UNICODE_STRING` 缓冲区的 NT DOS 路径转换。
- 目标执行到的 x87 整数加载/存储、浮点加载/存储、栈行为，以及乘/加操作。

无效的 `0x00000002` 控制目标只是次级 SEH 症状。保留的 `CALL`/`JMP`/`RET` 历史暴露了实际缺失的 `SetErrorMode` 导出，加载器现在通过自身的 `Import Error` 对话框路径报告未解析导入。一个已建模的 `psapi.dll` 为该诊断提供 `GetModuleBaseNameA/W`。

Kernel32 导出普查随后陆续越过了目录创建、文件属性、内存状态、本地时间、进程状态、区域设置、文件映射探测，以及初始线程 API 家族。如果文件映射与 `CreateThread` facade 实际被调用，目前会返回 `ERROR_NOT_SUPPORTED`；完整映射对象生命周期与多上下文来宾调度仍是明确的主线工作，而不是悄悄假装成功。

普查已继续通过固定的东京时区/系统时间数据、写入范围验证、沙盒化文件复制、与进程参数同步的可变标准句柄、ANSI 环境别名、区域设置验证/信息，以及真实的单区域设置宿主到来宾枚举回调。当前观察到的未解析导入是 `SetEndOfFile`。尚未创建真实 User32 窗口。HD 可执行文件仍作为比较路径有用，但不是主要目标。
