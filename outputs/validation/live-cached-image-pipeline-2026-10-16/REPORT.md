# Live 缓存图像链路：源码与协议验收（2026-10-16）

## 结论与边界

已实现 **Native 共享 PNG → Agent 只读侧通道 → MCP 组件专用 `_meta`**，并实际验证独立 MCP 子进程与固定证书 TLS 协议。**这还不是完成的 Host 播放器**：当前 HTML 仍是控制卡；会话内图像、PiP、host-only 不生成额外原生浮层、真实渲染/任务切换与严格 active-chat 绑定仍待实现/验收。本阶段未替换已安装 Agent/MCP，未重启 Codex，没有新建 GUI 窗口、捕获用户屏幕或发送输入。

同一产品源码根：`/Volumes/ChatGPT/Real World Interaction`。未创建开发源码副本；独立源码副本仅用于实际回滚验证。用户休息期间没有提出切窗、展开、密码或授权配合要求。

## 实际已做

1. **Native / Core**：`LiveController::latest_image` 与 typed query/cursor/reply；同一 `Arc` 共享 presenter pending 与 export PNG，避免额外复制。最多保留一份 4MiB、接受后 2 秒的当前帧，不重新捕获、重编码或降低图像分辨率。按已有 view/session/generation/sequence 和 presence 边界撤销缓存。PNG 检查仅覆盖头/尺寸，不冒称完整 CRC 校验。
2. **Agent**：私有 cached-only `computer.live_image`，不进入普通模型 catalog，不进入 Prepare/Execute 或 execution 图像保留池。独立 2-reader 配额；每 target 至少间隔 250ms，context 限流表最多 16 项；关闭和回包前复核。持有 input/executor 锁时仍能读取；不清 writer/fence、不续 presence 或 duration。
3. **MCP**：app-only `computer.live_panel_image`，仅有确切临时页能力与图片请求序列；不能选择 target、发输入或发布 presence。页隐藏/关闭/替换、过期后恢复、context/instance 变化都会使旧读失效。PNG 精确字节只进入组件 `_meta`，不进入模型 `content` / `structuredContent` 或 presentation cache。
4. **实际协议**：启动真实 rwi-mcp 子进程与独立临时 TLS synthetic peer，核对协商、能力拒绝、PNG 精确传递、unchanged 不重复图片、关闭撤销，以及图片 RPC 被刻意挂起时 hide/resume 能返回、迟到旧帧被丢弃。像素是 1×1 合成测试 PNG，绝非 GUI 兼容性证明。

## 同输入 BASELINE / MODIFIED / ROLLBACK / REAPPLIED

以下数字是匹配过滤器实际运行通过的用例数，**0 表示该功能测试在旧源码中尚不存在，不表示某个真实应用成功**。每列的命令和输入完全相同，四列退出状态均为 0；回滚在独立副本执行，还原原始全源码哈希及新增文件缺席后严格重应用补丁。

|范围|BASELINE|MODIFIED|ROLLBACK|REAPPLIED|
|---|---:|---:|---:|---:|
|F30 Core contract|0|3|0|3|
|F30 Native cache（纯状态测试）|0|8|0|8|
|F31 Agent|0|11|0|11|
|F32 MCP page / envelope|0|13|0|13|
|F33 真实 MCP 子进程＋TLS、synthetic pixels|0|5|0|5|

最终 Source470 全回归：**1280 passed / 0 failed / 11 ignored / 45 suites**；fmt、workspace tests、all-targets/all-features Clippy `-D warnings`、architecture 四项实际退出 0。架构检查仍列出 12 项既有债务；`architecture_complete=false`，没有提高行数上限。

复现命令（仓库根运行）：

```sh
rtk proxy cargo test -p rwi-macos --lib --locked --offline f30_live_image -- --nocapture
rtk proxy cargo test -p rwi-core --lib --locked --offline f30_live_image -- --nocapture
rtk proxy cargo test -p rwi-agent --lib --features fixture --locked --offline f31_live_image -- --nocapture
rtk proxy cargo test -p rwi-mcp --locked --offline f32_live_image -- --nocapture
rtk proxy cargo test -p rwi-mcp --locked --offline f33_live_image -- --nocapture
rtk proxy cargo fmt --all -- --check
rtk proxy cargo test --workspace --locked --offline
rtk proxy cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
rtk proxy python3 tools/check_architecture.py --workspace . --rules architecture/baseline.json
```

## 错误没有抹掉

- F30 首次 Native 编译退出 101：已有 recovery 测试需从新的 Arc pending 取出 owned VisualCapture；只修该测试调用。
- F32 首次 fmt 退出 1：`if` 后结构体 literal 需括号。随后 synthetic 安全整数负例首编译退出 101：数组被推断成 i32，明确为 u64 后继续；没有放宽产品边界。
- F33 初轮 4 通过、1 失败，退出 101：变化 instance 先被固定连接的 Native Link 拒绝，实际公开错误为 `live_image_transport_unavailable`，不是预计的 Gateway `live_image_response_revoked`。只修确切测试预期；无 `_meta`、仅 1 次 RPC、无重放断言保留。
- 原始 stdout/stderr、退出状态、失败源码/哈希及修正后的新命令均在对应 `work/f30` 至 `work/f33` 执行记录中保留；`MATRIX.json` 指向原始角色结果。

## 输出与耗时：不夸大

F33 单个 synthetic loopback 样本：原 PNG 70 bytes，模型 content＋structuredContent 共 252 JSON bytes，测试时钟取整 elapsed_ms=0。**不是** token 数、Native 捕获速度、Host 实际显示延迟、吞吐或优于 Computer Use 的证明。4MiB 是有限缓存/传输边界，超限拒绝而不偷偷降质；复杂真实画面的覆盖率尚待实际验收。

## 互联网参考与用户界面

官方文档定义了组件专用 `_meta` 和可选显示模式 API；这不等于当前 Host 已渲染或提供严格 active-chat 证明。[OpenAI plugin UI reference](https://developers.openai.com/plugins/reference)

官方 PiP 指南建议精简控件，并在会话结束时关闭。公开页面没有给出 Computer Use 预览的完整像素尺寸/按钮几何，因此不声称 1:1 外观复刻。[OpenAI UI guidelines](https://developers.openai.com/plugins/concepts/ui-guidelines)、[Computer Use](https://learn.chatgpt.com/docs/computer-use)

已有 Native 220pt 小预览与隐藏/X、44pt 命中区/28pt 图标、2048 源图像证据保持，未把截图降质当作缩小窗口。现有 `.ui-os/project.json` 是 Windows Pointer 草案，不能冒充本次 Mac/Host 播放器的已批准设计或完整 UI 验收。

## 下一项及完整目标

继续实际 Host 图像组件与无额外原生层模式，随后真实渲染、隐藏/恢复、关闭、任务切换验收；保留未完成的 Chrome 后台 Unicode/IME（既有 23/19 mismatch）、cold hover/modern tooltip、前后台/Command+Tab 冲突、modal/hidden、多窗口与分层性能/输出可读性目标。不重放旧输入，不把已完成的有限测试替换成全目标完成。

同一四角色（均已实际重开核对）：

- `/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/MODIFIED_FILE`
- `/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/DIFF_FILE.patch`
- `/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/VERIFICATION.txt`
- `/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/ROLLBACK.sh`
