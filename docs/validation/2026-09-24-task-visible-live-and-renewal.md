# 任务可见性、逻辑弹窗与 TLS 续签（2026-09-24）

## 验收定义与当前边界

用户要求的是「切走 Codex 任务即隐藏 Live，切回即恢复」，不是仅将 Live 生命周期绑定到 MCP 连接。本文分别记录 Host 信号、实现回归、部署及真实应用验收；一项成功不能替代其余项。

## 1. 已验证的 Host 可见性信号

- 通过 `open_in_codex` 在本任务打开有界 Rust loopback 探针，只记录该面板自身 `document.visibilityState`，不截屏、不发键鼠、不读其他任务内容。
- 用户确认「已切换并返回」。实际事件（UTC Unix 毫秒）：`1790260269498 hidden` → `1790260273607 visible`；随后 `1790260277199 hidden` → `1790260289454 visible`。说明该 Host 版本的任务内浏览器面板确实随任务切换产生可见性事件。
- 这比「任务被选中」更严格：关闭或用其他面板遮盖该浏览器面板，也会隐藏 Live。不是 Host task-ID 的可信证明，也不是复用 Codex 内置 Computer Use 私有叠层 API。
- 官方 [Hooks 文档](https://learn.chatgpt.com/docs/hooks) 明确指出切换任务不会立即触发 SessionEnd；[Plugin UI 文档](https://developers.openai.com/plugins/build/chatgpt-ui) 不构成第三方可直接控制内置 Computer Use 叠层的证明。实现不修改 Codex、不调用私有 IPC、不抓取其他任务。

## 2. Live 实现与离线回归

- `computer.live start` 返回当前任务应打开的 `viewer.url`。Native overlay 初始隐藏；可见性有 1500 ms 的短期租约，每 500 ms 可见心跳刷新。失联、隐藏或超时清空画面并隐藏，恢复可见不覆盖用户 Pause/Stop。
- 分离随机 URL token、设备 binding 和页面 generation；顺序号拒绝重复/晚到事件。页面重载撤销旧页面，Stop 撤销 URL，MCP EOF 关闭 loopback listener。事件没有键鼠权限。
- 固定 loopback、精确 Host/Origin、CSP nonce、无 CORS、请求长度/连接数/超时限制；页面不提供屏幕图片或密钥。回归覆盖外部 Origin、旧页面、乱序、模型直接伪造内部 view 参数、Stop/EOF 及无效化。
- **尚待真实 Live 闭环**：上面的用户切换是纯可见性探针，不是新实现的 overlay 验收。需重新加载新 MCP 后测切走/返回及长操作时的延迟。心跳当前仍走已认证 Prepare/Execute 控制流程，其与长操作争用可能导致保守隐藏，不能凭单元测试声称流畅度已达标。

## 3. 独立逻辑 AX 弹窗

- 既有实机证据：Paint 弹窗渲染 CG ID 与 `AXFocusedWindow` 逻辑 ID 不同；逻辑窗口不在 on-screen-only 清单。见 `outputs/validation/2026-09-24-live-acceptance/paint-dialog-native.txt`。
- 新实现单独发现 `semantic_only` 对话框：同用户/进程出生身份、当前 focused dialog、精确 AX 原生窗口 ID、应用级中心 hit 的同一 AXWindow、全 CG 清单的同 ID/PID/几何、多次回读一致。没有把另一 CG ID 当作别名。
- `snapshot` 可返回有效 frame/AX controls 而不伪造截图或像素映射；这类窗口只接受明确暴露的 AX action。绑定它不改变之前明确选定的 Live 可视源。自身 AXModal 不再被误判为另一个阻塞窗口。
- 离线集成测试确认 semantic-only snapshot 不请求 `computer.image`；**真实 Paint 该类弹窗的点击与结果验证仍待部署后执行**。

## 4. 经明确批准的可续签 TLS 配对

- 用户确认「同意迁移为可自动续签配对」。只迁移 RWI 传输 TLS；macOS `RWI Local Development` 代码签名证书仍有效至 2036-09-16，SHA-1/DR 不变，不导出钥匙串私钥。
- 新配对显式使用 `spki:` SHA-256 pin。旧 64 hex pin 仍表示整张证书，不因升级隐式放宽。完整 WebPKI 链、leaf 时间、用途、服务名及握手签名验证先于 pin 检查；有歧义的 principal 匹配拒绝。
- 一次性迁移新建可续签私有 CA，并复用现有 agent/controller/model 端点密钥。CA 签发私钥仅保存在本机批准的私有目录，0700/0600、不进源码/证据/APP、不改系统信任。
- 部署前额外发现旧去重账本按 leaf pin 分区，因此增加明确的 `journal_identity` 迁移字段：新 SPKI pin 用于认证，旧身份仅继续作为账本/上下文命名空间。配置拒绝身份冲突；原证书 pin 不因这个映射重新获得认证权限。fixture 实际派发两次后迁移并重启，旧请求仍拒绝重放，计数保持两次；历史记录文件不改写。
- leaf 180 天有效，到期前 30 天续签；Agent 启动/每小时检查，长期运行的 TLS resolver 在下一握手前也会检查。新握手按原端点 key 加载新 leaf；常规续签不要求重启 Codex、不重建操作 journal、不重放输入。
- CA 有效期 3650 天。CA 轮换仍需显式信任迁移；过期不能跳过。Windows 仍为原配对，本次未唤醒 VM，也未声称实现远端自动发证。
- 预创建稳定 lock inode，跨进程有界互斥，逐文件原子替换/fsync。中途只更新部分角色可在下次检查继续，因为 CA/端点 key 不变。并发回归最初在 lock 获取路径返回 NotFound；改为 bundle 提交前预创建、运行时只打开，连续 20 轮通过，不据此宣称已定位 macOS 内核原因。锁 API 边界参见 [Rust File::try_lock](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock)。

## 回归与部署状态

- 最终：`fmt --check`、全仓 `test --locked --offline` **505 passed / 10 ignored**、Agent fixture **44 passed**、全 targets/features `clippy -D warnings`、`git diff --check` 和 release 构建通过。没有提交/推送或触发 GitHub Actions。
- TLS memory-duplex 测试使用同一组 client/server config，续签前后完成真实 TLS 1.3 双向握手，并读回不同的新 leaf 指纹；端点 key/pin 保持一致。另有实际 loopback RPC 验证服务端传给业务层的是配置中的 SPKI principal，而非变动的 leaf hash。
- 反例覆盖过期/未来时间、错误用途、非配对 key、外部 CA、CA 私钥不匹配、路径/链接、根轮换、重复配置等。CLI 成功输出只有公开状态。
- **已部署并读回**：Agent PID `57373`，instance `5b0f2e4c-f765-4734-a70c-7c060ed884ae`；Agent SHA-256 `f5522bb92a0ca4ac8fcfdce41d2f96d8126521e80f31cb573d417308b19a2d47`，MCP `4b83f24af1ba38c116d1973ee930c632b6b3d93e75002b07b7f13a66155b1cfa`。Plugin `0.2.0+codex.20260924154944` 通过 CLI 重装；没有手改 marketplace 或无关 Plugin。
- 新启动的实际 installed MCP 使用生产 Host profile，mac-host healthy、34 个工具、新版 Live 描述与内部 view 字段隐藏均已读回。四项 TCC（AX、screen_capture、listen_event、post_event）仍 true，没有再次申请授权或改代码签名 DR。状态/Windows profile/旧账本摘要验证不变。完整全设备 targets 用时 5006 ms，包含离线 Windows 的 5 秒连接超时；**不是 mac-host 单步原生操作耗时**。
- 当前任务已加载的旧 MCP 查询仍返回 `BadSignature`，而上述新进程成功；旧 CA 仍驻留于旧进程。用户选择「稍后重启」。不终止其他任务的 MCP，不继续用旧 target 发送输入，不将 standalone 传输成功冒充为 Host 原生 GUI 验收。
- 新 leaf 到期 `2027-03-23 15:40:32 UTC`，计划续签点 `2027-02-21 15:40:32 UTC`；根到期 `2036-09-21 15:40:32 UTC`。公有元数据与部署读回见 `outputs/validation/2026-09-24-renewable-pairing/`，私钥仅在安装根私有目录，未复制进证据。
- Paint V14 工作源与交付源均为 446815 bytes，SHA-256 `1573f4f74b45a6c13014dd9c0efeb1d2d3a2185f9fa4368edab74c91ba8a9998`，与上次保存一致；本轮未新增绘图或宣布复刻完成。

## 重启后的明确接续

1. 加载新版 Skill，重新 `targets/discover`；旧 context/target ID 不再可用。Windows 保持离线，不唤醒。
2. 在已分配的 BetterDisplay 工作区重新绑定 Paint，启动 Live 并把新 `viewer.url` 用 `open_in_codex` 打开在本任务。分别验证切任务、返回、遮盖/关闭面板、Pause/Stop/到期以及长动作期间的时序，记录真实截图/原始回执，不只采样页面事件。
3. 用真实 Paint modal 验证 semantic-only 发现/观察/AX action 与独立视觉验证，不能用另一个 CG ID 的像素替代逻辑窗口。
4. 继续优化已确认的耗时/输出问题：全设备 targets 受离线 Windows 影响、部分 macOS capability 文案（modal/pointer feedback）滞后，以及 Live 心跳与长操作争用。上述问题不因本轮续签完成而关闭。
5. 随后继续 V14；持续保存可编辑 `.paint` 源及实际截图，跨重启 Undo 栈仍未证明。

补充：Jev 工具本轮返回限流，没有取得其允许/完成裁决；执行依据为用户明确授权、手动边界复核和上述实际回归，不声称外部评审已通过。架构 ratchet 脚本缺少显式 rules 路径未运行成功，未算入通过门禁。

后续进展：用户已重启 Codex；实际 Live 任务切换、Retina 2048px 清晰预览及 Paint 普通 Resize 弹窗验收见 [2026-09-25 记录](2026-09-25-live-clarity.md)。本页上述“待重启/待验收”为当日历史状态，逻辑 `semantic_only` AXDialog 仍待单独实测。
