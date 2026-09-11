# M1：Windows Rust 原生观察与映射

此文档描述 `0.2.0` 候选实现。M1 与 v0 证据分开；**不是 W01-W12 完整任务验收报告**。Windows 输入和强制中断在途原生调用均未实现。最新构建与测试身份见 `outputs/m1-windows-observe/verification.json`，真实执行另存带 run ID 的目录。

## 实现与边界

| 层 | 实际实现 | 未提供或未验证 |
|---|---|---|
| 原生执行身份 | MachineGuid 哈希、TokenStatistics AuthenticationId、WTS session/logon time、WinSta0/thread/input desktop、一致且 unlocked active session | Windows 内无法原生证明物理 Mac 与 Parallels VM UUID 的关系；外部 setup 映射单独记录 |
| 窗口 | EnumWindows、HWND/PID/进程创建 FILETIME/class、GetWindow owner chain、父级、foreground、可见/最小化/enabled/cloaked、DWM 与 GetWindowRect 分开 | 没有 WinEvent HWND generation tracking；未知 epoch 不绑定；目录最多 256 项，竞态/截断不能当完整扫描 |
| Popup | 10 秒 native catalog ticket；initial / owned_popup / return_to_owner；原生 owner 的 HWND/PID/创建时间匹配；重绑清除旧 frame/reference | 不按标题、同 PID 或坐标猜所有者；不支持在同连接中任意切换无关系应用；owner 不是 modal 的充分证据 |
| UIA | 每次调用内 MTA、CUIAutomation8 / IUIAutomation2、禁止 AutoSetFocus、250 ms connection/transaction timeout、控制树 cache、runtime ID、WindowPattern modal、ValuePattern 精确核验 | 128 节点 / depth 8 / 1500 ms 协作预算；partial 标记；原生 RPC 或 provider 分配没有硬总时限/硬内存上限 |
| 可见性 | native WindowFromPoint + UIA ElementFromPoint 与最多 8 层 ancestor 比较 | 只证明候选中心点，不证明整个控件表面；未知结果不变成无遮挡 |
| 显示 | PMv2 thread scope，EnumDisplayMonitors、MONITORINFO、DEVMODE 物理尺寸/旋转、window awareness-qualified DPI、负坐标 | 不猜 effective monitor DPI，不重复乘缩放；跨屏/窗口不完全包含于单屏拒绝；未运行的拓扑不计验收 |
| Capture | GetDC(desktop)、CreateDIBSection top-down BGRA、BitBlt SRCCOPY/CAPTUREBLT、GdiFlush、RAII 清理；DWM visible frame 是全局物理裁剪，不混用 PrintWindow buffer | 可见桌面像素；遮挡层保留；HDR、受保护内容与光标保真未保证；不恢复/聚焦/改分辨率 |
| 原图与视图 | 32 MiB/frame、64 MiB 总原图缓存、8 MiB raw inline；Rust PNG 编码；明确 preview/tile 和 pixel-center affine，直接 MCP image content | 缓存会按预算淘汰，关键原图需及时归档；host 渲染/传输成功必须另证，不能从编码成功推断 |
| 取消 | operation_id、cancel/operation 不占 engine gate；逐阶段/逐行/逐节点检查；native call pending 与 cleanup 分离；异步 waiter drop 请求取消 | 不能抢占在途 COM/GDI；取消确认不等于 worker 停止；Windows 没有输入，所以未验证“停止后不继续打字” |
| Guest transport | 一次连接的 Rust 本地 named pipe；user+SYSTEM DACL、拒远端、first instance、1 client；guest client 校验 server PID 对应磁盘 image SHA；无 reconnect/replay | 不自动启动 scheduler/VM；真实 Parallels 传输与 Codex 发现需单独测试；磁盘映像 SHA 不等于独立的加载内存完整性证明 |

## 构建和运行

所有 backend/MCP/pipe 代码为 Rust。Mac 上交叉链接：

```sh
rtk proxy cargo xwin build -p rcu-mcp --bins --target aarch64-pc-windows-msvc --release --offline --locked
```

不是 x64 仿真，不把 macOS test/check 当 Windows 原生执行。默认 `rcu-mcp --state-dir <absolute-path>` 注册 0 targets，不查询/控制桌面。Windows 必须在新鲜分配的交互 session 中明确启动：

```text
rcu-mcp.exe --state-dir C:\Temp\RealComputerUse-M1-<run-id>\state --windows-desktop --pipe-server <run-id>
```

pipe 只接受本机；等待首个客户端 120 秒。Mac 侧可用 Parallels Guest Tools 承载 Rust guest client 的 stdio，Parallels 只是 setup/字节通道，不能用它的 screenshot/input 替代被测操作。候选不改旧 CrossMacro、VM launcher、共享剪贴板/文件夹、全局 Codex config 或系统权限。

新插件在 `plugins/real-computer-use-m1`，v0 `plugins/real-computer-use` 保持原样。新插件默认也为本机 0 targets。没有创建个人 marketplace 或全局安装。项目局部 MCP 配置、实际 app pickup、工具发现与 model 调用需各自记录；仅 `tools/list` RPC 不能记为 Codex 自主选择工具。

## 使用序列

1. `computer.targets` 检查当前执行 OS/session/capabilities；不要推断 guest 路由。
2. `computer.discover {target,operation_id}`；选取具体 HWND/PID/native_epoch。
3. `computer.bind_window {target,selection:{window,pid,native_epoch,relation:"initial"}}`。目录过期先重查。
4. `computer.observe {target,operation_id}`。可选 `region:[x,y,width,height]` 是相对 native window capture bounds 的原生 ROI；不是 preview tile。检查 source bounds/normalization、UIA partial 与 native_details。
5. 原图大于 inline 阈值时自动附明确标记的 preview。可调用 `computer.image {target,frame,artifact_id,crop:[x,y,w,h],max_edge:1200}` 请求 native tile 或 preview，使用 `computer.resolve_view`，不是把预览坐标当原图坐标。
6. semantic 同名时使用 `computer.resolve` 的 `locator:{kind:"element",id:...}`；参考绑定的是具体 runtime ID 和 frame。
7. M1 Windows 不输入。`computer.verify` 只是读取当前精确 predicate，不能将已有值等于期望解释为完成修改任务或因果证明。
8. `computer.cancel {operation_id}` 后通过 `computer.operation` 确认 terminal/resources_released。pending 时不开始冲突工作。恢复后全新 observe/resolve。

## 错误阶段

- `native_capture_failed` / DC/bitmap/flush errors：原生采集阶段。
- `encoding_source_layout` / `png_encoding_failed`：布局或编码阶段。
- `png_size_limit` / `screenshot_size_limit`：32 MiB 原图限制；不自动换分辨率。
- `view_transport_size_limit`：导出视图超过 inline budget；原图不变，可请求更小 preview/native tile。
- `artifact_frame_mismatch` / `artifact_dimension_mismatch` / `unknown_view`：不能使用映射。
- stdio/pipe/host rendering 错误：独立 transport/UI 层，不反向伪称 capture 失败或成功。

## 验收次序

1. 默认离线：既有 no-replay、identity、scope、lease、预算与恢复；MCP actual process；取消 lifecycle；高熵 3600×2186 与 1200×600 ROI/图像映射。
2. ARM64 guest 只读 native smoke：新鲜交接 -> 独立候选安装/启动 -> 实际 identity/windows/UIA/capture/ROI -> 保存原图、展示 view -> 退出/归还。
3. Mac Codex 发现并调用新工具，记录 exposed tool set、实际模型调用与图片可见证据；与脚本 RPC 分开。
4. 输入实现之后，才运行 discover app -> menu/settings -> popup -> modify+independent verify -> restore+verify 的完整目标流程，以及 W01-W12 干扰/取消/断线/恢复。M1 不跳过未实现的动作后计为通过。

## 官方接口依据

- [WTS session state](https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/ns-wtsapi32-wtsinfoex_level1_w)：当前 Win11 active/unlocked 判定；不外推旧 Win7 标志语义。
- [GetDpiForWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getdpiforwindow)：结果依赖目标 window 的 DPI awareness，所以单列而不当 monitor DPI。
- [GDI capture](https://learn.microsoft.com/en-us/windows/win32/gdi/capturing-an-image)：native DC/bitmap 操作；本实现不使用示例的缩放。
- [UIA transaction timeout](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation2-get_transactiontimeout)：RPC timeout 配置不是可抢占的总 deadline。
- [Codex MCP 配置](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)：可信项目 `.codex/config.toml` 可配置局部 MCP；工具实际 pickup 必须现场确认。
