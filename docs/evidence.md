# 本轮证据与问题映射

## 工程归属与保留关系

- 正式工程：`/Users/vincentadamnemessis/AI Projects/Real Computer Use`，独立 Rust workspace、独立 `main`，没有 commit/remote/push/tag。
- 历史参考：`/Users/vincentadamnemessis/Documents/Codex/2026-09-10/crossmacro-mcp-reliability/outputs/CrossMacro`，完整旧 Git、C# 改动与 Python 原型留在原地，不作为新产品运行链。
- 上游锚点：[CrossMacro 固定提交](https://github.com/alper-han/CrossMacro/tree/c13c93163478848655d86695385400876c9a5200)，`v1.4.0`，SHA `c13c93163478848655d86695385400876c9a5200`。
- `reference/source-preservation.json` 对 2,327 个参考源码/文档文件建立索引；不搬 `.git`、SDK、缓存、二进制、私有目录、截图或完整会话。源码树索引摘要 `53576ec00af37e1830e97707039b9f526d37a149b5050f308bdaefbf9d21b432`。
- 安装目录中明确允许读取的 14 个源码/备份文件重新取 SHA-256，全部与最初基线一致；对应脱敏快照哈希也全部一致。该结论覆盖这 14 个文件，**不是宣称检查了安装目录所有文件**。

## 历史审计口径

沿用父任务按 `item_completed` 唯一事件 ID 提取的审计，时间范围从 2026-09-08 UTC 到最后可见事件 `2026-09-10T07:17:21.773Z`。没有重新扫描全部 1.19 GB 会话，也没有把关键字搜索候选当成实际绕过。

| 证据面 | 已证实的计数 | 不能推出的结论 |
|---|---|---|
| 原生工具事件 | 323 次：Debian 216、Windows 105、macOS 2；截图 82、命令 183 | 不是任务成功率，也不覆盖未读子任务 |
| 原生调用失败信号 | 11 个 | 不是 11 次业务目标失败 |
| Windows Python MCP 客户端记录 | 104 个 JSON、90 个会话；checkpoint/final 去重后 2,053 次 MCP tools/call，11 个失败信号 | Python 客户端本身不是绕过 MCP |
| Python MCP 工具分布 | command.execute 1,272、screenshot.capture 487、window.query 237，其余见原索引 | 不把文件数等同于调用数，不把记录缺失当成功 |
| 经源码/输出确认的截图回退 | 4 次 `prlctl capture`，2026-09-10 02:12–02:21 UTC | 不能用所有 `python` / `prlctl` 关键字候选估算回退比例 |

原始可核查索引位于旧参考目录的 `reliability/audit/native-summary.json`、`python-mcp-summary.json`、`historical-failure-cases.json`、`historical-receipts.json`。其父任务来源摘要 SHA 已在这些文件中记录；没有把敏感原会话复制到正式项目。

## 从真实失败到修复，而不是继续堆宏脚本

| 观察到的问题 | 本轮处理 | 验证／边界 |
|---|---|---|
| Windows `Transport closed`，客户端/服务进程生命周期不清 | 原安装 raw-stdin relay、初始化前有限重试、90 秒 lost-client 清理等完整保留；Rust MCP 使用官方 SDK，EOF 可退出，输入有落盘 reservation | 旧 relay 6 项参考回归；Rust 实际 stdio 进程 + 断线后不重放测试。Windows Rust broker 尚未实现 |
| 模型猜 `Win+D`、`Shift+Minus`、`command=help`、`minimize+address` 参数 | 旧 C# 候选补 usage/参数发现与键别名；正式 MCP 改为 8 个强类型 intent 工具，拒绝未知字段；macOS 有明确物理键表 | 不宣称 macOS 的 Command 别名等于完成 Windows Win 键实现；不把 minimize 替换成 close |
| 完整 3600×2186 截图失败，回退图约 13.9 MB，而 1200×600 区域可成功 | PNG 大小与 inline 大小分离；Rust 单图 16 MiB、inline 8 MiB、总缓存 32 MiB，保留原尺寸及显式 normalization、hash 与分块读取 | “8 MiB 导致当时失败”仍是证据支持的原因假设，未恢复到当时精确异常；实际原生大桌面验收 pending |
| Debian `outputPath` 被 FileWrite 策略拒绝 | 不扩大权限；新 MCP 图像只在内存，artifact 不写任意路径或剪贴板 | 历史策略拒绝不等于截图 API 坏了；本轮没唤醒 Debian |
| GNOME promise/tuple 返回 ABI 不匹配 | 旧补丁按 `GLib.Variant('(sib)', ...)` 重建并保留 | 原始 JS SHA `eccb2405cecc64cb5169b9a6ff27d1e13eae8d0d2479f84177a91bfa9720549b`，补丁 SHA `a44be48e3902cebef9fab2994a5e3aa0e071053c0a64e44130ceb36bf8429aff`；没有把 JS 当成 Rust Linux 实现 |
| 滚动后继续点击旧 Clear 坐标，实际漏点 | 10 秒 frame、目标身份/状态/位置、焦点/布局/模态 epoch、native hit-test；动作后使同桌面别名引用一并失效 | `stale_after_scroll`、布局变化、元素移动/遮挡和别名失效反例 |
| “输入成功”被误当“目标完成” | act 回执永远 `goal_verified=false`；verify 独立读取精确窗口/PID/元素；可选 request provenance | 完整合成设置修改→验证→恢复，ACK 无效果、同值异窗口/PID/元素、错误 provenance 反例 |
| 全帧 hash 把 caret/无关动画视为目标变化 | semantic 走原生 metadata revalidate，绑定选定元素状态而非无关像素；pixel-only 仍保守比较全帧 | 正反对照测试保留元素身份、位置、值状态、焦点、布局、模态和遮挡约束；原生性能仍待 live 验证 |
| 原生 secure text 分类写错 Role/Subrole | 读取 `AXSubrole`，统一 `TextClass` 控制描述读取、输入、验证；unknown（包括 AXUnknown）不猜，pointer/key-chord 不绕过分类 | 5 个纯测试含 `AXRole=AXTextField + AXSubrole=AXSecureTextField`；按本机 Apple SDK `AXRoleConstants.h` 校验 |
| 并发 fork 后仅 close 导致锁短暂残留 | Lease Drop 显式 unlock，再 close；TTL worker 与 dispatch guard 共用锁 | 真实子进程排斥、子退出不能释放父锁、release/drop/无析构异常退出后可重新获取 |
| CGImage 重绘会改变颜色/alpha，却称原始像素 | 明示 device RGB / straight RGBA normalization，不缩放；回调 panic 转为错误而非跨 FFI unwind | 离屏 CGImage→CGContext→PNG 2×2、透明/非 opaque alpha、行向/通道/尺寸与回调异常测试；不是截屏 |

## 回归证据分层

正式 Rust 回归详见 `outputs/verification.json` 与 `outputs/rust-tests.log`：核心行为、macOS 离线/离屏路径、MCP schema、真实 stdio 子进程分别统计。唯一 ignored 是 `tests::lease_child` 子进程入口，由父测试以 `--exact --ignored` 实际启动；它不是被隐藏的原生平台验收。

冻结参考的 .NET SDK 10.0.100 回归：MCP **203 passed / 0 failed**；Infrastructure **1,609 passed / 0 failed / 4 skipped**，合计发现 1,613 项。TRX 路径、SHA 和原始 counters 已写入 `reference/source-preservation.json`。旧 Python intent 原型 **20 passed**，仅用于迁移对照，不计入 Rust 测试数、不进入生产运行链。

SDK 与协议来源：[官方 Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)、[Codex MCP 文档](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)、[插件文档](https://developers.openai.com/plugins/build/plugins)。`implemented`、服务策略允许、客户端实际暴露是三层；新服务器对客户端暴露状态明确返回 unknown，不用实现工具列表替代它。

## 本轮没有做的操作

没有实际屏幕采集、鼠标/键盘输入、窗口激活、权限申请、Windows 控制权切换、Debian 唤醒、全局插件安装、Codex 配置修改、Git 提交/推送/tag。正式 Rust 原生 macOS 已编译，但 live 完整目标验收仍为空；Windows/Linux/远端/VM 原生实现仍为空。详细剩余门槛见 `architecture.md`。
