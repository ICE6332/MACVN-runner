# 目标：euphoria HD

这是第一条以目标驱动的兼容性追踪记录。商业可执行文件与资源文件均保留在已忽略的 `tests/targets/` 目录下；本文件仅记录技术元数据与 Runtime 运行观察。

## 本地目标指纹

- 主映像：`tests/targets/euphoria/inspect/euphoriaHD.exe`
- SHA-256：`c0e3aa244cd145951804a0cde2d9ce4f60b4dc5cabd5f1c720719cf2180aec4a`
- 格式：PE32 GUI，Intel 80386
- 首选映像基址：`0x00400000`
- 入口点：`0x004652c6`
- 映像大小：`0x00590000`
- 基址重定位：无
- 静态导入：258 个

按模块统计的导入数量：

| 模块 | 导入数 |
| --- | ---: |
| KERNEL32.dll | 125 |
| USER32.dll | 76 |
| GDI32.dll | 18 |
| WINMM.dll | 18 |
| SHELL32.dll | 7 |
| ADVAPI32.dll | 4 |
| ole32.dll | 3 |
| VERSION.dll | 3 |
| COMCTL32.dll | 1 |
| d3d9.dll | 1 |
| DSOUND.dll | 1 |
| IMM32.dll | 1 |

该映像通过序号导入了 `COMCTL32.dll!#17` 与 `DSOUND.dll!#1`。Runtime 的序号绑定使用稳定的 `dll!#ordinal` 宿主调用键，已不再阻塞映像加载。

## 动态兼容性队列

本地运行时集合当前包含主 EXE、必需的根配置文件与 DLL，以及 3.1 GiB 的活跃 `pac/*.ypf` 归档。冗余的 `cg.ypf_old` 备份与嵌套补丁归档尚未解压。

在真实执行路径上已观察并顺利通过：

1. Win10 版本检测、CRT 启动、私有堆、递归临界区以及动态 TLS 初始化。
2. CP932/UTF-16 转换、CTYPE 表、大小写映射、进程/系统信息、x87 控制字设置，以及保守的 CPU 特性检测。
3. 命名互斥体、事件、等待、PE 资源、AppData/Documents 查找、COM 套间初始化，以及通过真实 `WIN32_FIND_DATAA` 记录进行的根文件枚举。
4. 通过 User32/GDI32 探测显示能力，包括可选的 `GetProcAddress` 回退，随后是目标观察到的 MMX 表填充序列。
5. WinMM 启动清理命令（`stop/close ysmcimovie`）与旧版全局内存清理。

Runtime 现在会挂起宿主调用，通过受保护的返回哨兵进入 stdcall 来宾回调，支持嵌套同步回调，并恢复原始宿主调用者以返回请求结果。真实对话框过程成功处理了 `WM_INITDIALOG`、嵌套的 `SendMessageA`、`WM_COMMAND/0x40A` 以及 `EndDialog`。

追踪真实对话框后确认，它是引擎的许可证信息对话框，而非图形设置对话框。原始目标包不包含必需的 `system/YSCom/YSCom.exe`；该文件在外层 ZIP、两个嵌套补丁归档以及每个已解压的 YPF 目录中均不存在。引擎将其视为致命初始化失败，显示许可证对话框后，以退出代码 0 执行干净关闭。当前已支持配置的来宾根目录下的绝对路径，以及 Windows 风格的不区分大小写路径查找，因此 `euphoriaHD.exe`、`yscfg.dat` 和 `yssfs.dat` 均可找到；剩下的 `YSCom.exe` 失败是真实目标素材缺失。

下一步目标：从完整安装中提供匹配的 `system/YSCom/YSCom.exe`，重新运行现有追踪，仅从新执行的调用扩展 User32，并在 `CreateWindowExA` 被触发时挂载第一个 SDL3 窗口。

存在静态导入并不意味着每个 API 都必须实现。API 仍保持目标驱动，仅当执行路径实际到达时才会添加。
