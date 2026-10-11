# 定向设备发现与 Mac 能力元数据（2026-09-25）

## 真实问题与边界

- 当前 Codex 任务的 `computer.targets({})` 返回 mac-host 健康、win11-vm 离线，但整个调用约 5 秒；延迟来自等待无响应的 Windows 端点，而非 Mac 原生单步操作。两个设备仍是独立配对和上下文，不能把 Mac 结果冒充 Windows 状态。
- 原 macOS capability 同时报告已实现的 pointer feedback 又在 `unsupported` 写“Pointer feedback”，仍把 `live_acceptance` 标为 pending，并笼统声称模态窗口输入不支持；与前一天实机的普通 Resize 弹窗和本日 Live 验收矛盾。

## 实现和回归

1. `computer.targets` 增加可选 `device_id`。省略仍并行检查所有配置端点，保持旧语义；提供已知的精确配对 ID 时只联系该端点。无效或未配对 ID 直接返回明确错误，不扫描其他端点，不自动唤醒 VM，也不把缓存目标当新鲜事实。JS SDK 的 `targets(deviceId)` 与插件入口 Skill 同步支持。
2. 双端 fixture 的真实 MCP 进程测试确认：请求 `device-a` 只返回其目标/状态，`device-b` 没收到任何 RPC；无效/未知 ID 不触碰后者；不带 ID 时两端都检查。JS SDK 测试确认所选 ID 被原样转发。
3. 生产 Host profile 的**独立新 MCP 进程**读回：release 构建定向 mac-host **16ms**，全设备 **5003ms**；部署后的安装版定向 mac-host **47ms**，全设备 **5002ms**。这是设备列表端点的两组实际样本，不能推断 Mac GUI 截图或输入也同幅提速。Windows 保持离线，未启动/唤醒。
4. macOS capability 现准确区分：on-screen CG 清单及 exact-PID 查询可能额外发现无像素的 focused semantic AX dialog；只允许该弹窗明确 advertised 的 native AX action，另一被阻塞窗口/未知模态仍 fail closed；pointer feedback 标为已实现；Live 的当前成败须查询每次 session status，不能用静态 `pending` 字段推断。`unsupported` 移除这两项已不真实的声明，仍保留未实现的激活/启动/owner-chain 等。底层授权和动作策略未在这项元数据修正中放宽。
5. `computer.js` 的 `window.snapshot({preview:true})` 默认缩略上限跟直接 `computer.snapshot` 对齐到 2048px，避免不同入口静默退回 1024px；`captureVisual` 的快速 response-only 默认仍为 1024px，可显式请求 2048px。

## 部署及 Host 读回

- 全仓 `cargo test --workspace --locked --offline` **508 passed / 10 ignored**，`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`、`git diff --check` 与 JS SDK 44 项通过；Agent/MCP release 构建通过。
- 本机 Agent 原子替换并沿用原 designated requirement/本地证书，安装 Agent SHA-256 `7fbb9a932e89d89d47606a46de58a1ee2a6cce8c63db4c64e2de9cd0c9ce5f19`；安装 MCP SHA-256 `f452e45b4b1bb91cb364393f3f5fc3c9367204fd85a0b7427dc89d180b37fca2`。Agent 配置和 client profile 摘要未变，旧 app/binary 留在本机私有回滚目录。
- **真实 Codex Host 旧 MCP 会话**在 Agent 重启后自动获得新 mac target，实读四项权限 `accessibility/listen_event/post_event/screen_capture=true`，新 `semantic_dialog_discovery`、`modal_policy`、`pointer_feedback.implemented=true` 与 `live_acceptance=per_session_status_required` 均已出现。重新绑定原 Paint S 窗口后，Live 在 BetterDisplay 虚拟屏恢复：源 2048×942、面板 visible 且有图、无错误。当前任务加载的旧 MCP schema 仍不接受新 `device_id` 参数；新安装进程已经通过定向探针，新任务/Host 重载后才能直接调用该参数。
- Plugin `0.2.0+codex.20260925031512` 已按本地 marketplace cachebuster 流程重新安装，源 Skill/安装 Skill/Plugin 均验证通过。没有修改无关 Codex 配置、未提交/推送。
- 两份磁盘可编辑 `.paint` 源仍各 446815 bytes，SHA-256 `1573f4f74b45a6c13014dd9c0efeb1d2d3a2185f9fa4368edab74c91ba8a9998`。本轮只观察 Paint，没有新增绘图，也不声称 V14 完成。semantic-only AXDialog 的实际点击闭环、Live 长操作争用及 V14 仍待后续验证。
