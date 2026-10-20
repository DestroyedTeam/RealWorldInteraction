# RCU Session 接续包（2026-09-15）

> **用途**：把 opencode session `ses_f61ed8b9affeZFPndcqjhZmaXk`（标题「Real World
> Interaction」，目录 = 本仓库根）的上下文迁移到一个**新的、可继续的会话**。
> 原始完整历史存档于 `work/session-migration-20260915/`（93 条用户消息全文、504 条
> assistant 文本、1012 次工具调用元数据）。本文件是**精炼接续面**，不是历史全文。
>
> **接续方式**：新会话只需读取本文件 + `docs/product/implementation-status.md`，即可
> 恢复可工作状态，无需加载 20MB 原始历史。

---

## 0. 为什么之前的迁移会"上下文太大"（原始会话自证结论）

来自原 agent 用 opencode 会话库自身的量化（15:26:29）：

| 项 | 数值 |
|---|---|
| 原始会话 `part` 表 | 4704 条；tool 输出 **14.7MB**、reasoning ~2.7MB、图片附件 ~2.1MB、text ~190KB |
| 元凶 | **tool 结果（截图像素 base64 + 大 JSON）** 与巨型 skill 文本，**不是用户对话** |
| 一次性全量迁移 | 必然溢出，这就是"报错上下文太大"的根因 |
| 压缩后 | 对话尾部只剩 ~5 条工具输出（12KB）；**压缩机制本身正常** |
| "other 占 85%" 的真相 | other ≈ 系统提示 + AGENTS.md + 24 个 skills 元数据 + **三个 MCP 服务器约 126 个工具 schema**（proxyman ~70 个本项目根本没用）。底盘约 45–50k tokens，压缩后对话仅 ~9k → 一刷新就是 85% other |
| 已下的药 | 本仓库 `opencode.json` 加 `"proxyman": {"enabled": false}`（**需重启 opencode 生效**）；14MB 历史不背锅，它只在磁盘 |

**结论**：迁移必须**剥离 tool payload、剥离巨型 skill 文本**，只保留「用户需求 + 里程碑
结论 + 当前代码断点」。

---

## 1. 项目与当前状态

- 项目：Real Computer Use（RCU），Rust workspace，`crates/{rcu-core,rcu-agent,rcu-mcp,
  rcu-macos,rcu-windows,rcu-imaging,rcu-js,rcu-link-trust,rcu-native-link}`。
- 架构基线：**ADR 0002**；主状态表：`docs/product/implementation-status.md`（以它为准）。
- 本机 macOS agent 已部署并运行：`~/Library/Application Support/RealComputerUse/agent-mac-v1/`
  （`bin/rcu-agent --config .../agent.json`）。
- Windows VM 已配对：`.rcu/profile.json` → `win11-vm` @ `10.211.55.3:45128`。
- MCP 工具目录：19 项固定工具（`computer.*`）。
- 授权显示器策略已生效：只在 **MacBook 内建屏**（`built_in:true`）操作；`place_on_bind`
  默认 **false**（2026-09-15 隐式移动事故后改为显式）。

## 2. 当前断点（工作树未提交，`cargo check` 通过）

**已提交**：`3baeb67 feat(computer): explicit window move primitive across core/agent/mcp`
（#3a 显式窗口移动原语）。

**未提交的工作树变更**（12 文件，+179/−22）——这是#3a 验收过程中发现并修复的两个问题：

1. **单向陷阱修复**（bind 拒绝非授权屏窗口 → 窗口被困住）：
   `place_on_bind=false` 时 bind 不再隐式移动、也不再拒绝（选择与移动解耦）。
2. **安全洞修复**（窗口级引用竟能过 `act`，在窗口中心派发点击）：
   - 新增 `Locator::Window`（窗口级引用，无元素、无输入级可见性门槛）。
   - `Resolved` 新增 `placement_only: bool`；`Locator::Window` 置 true。
   - `Engine::act` 拒绝 `placement_only` → `invalid_reference_class`
     （`crates/rcu-core/src/lib.rs:1000`）。`act_checked` 走元素路径，不受影响。
   - `move_window` 补齐 `request_id` + 持久预留/去重台账（与 act 同款 no-replay）。
   - 三处后端补齐 `move_window`：`rcu-macos/src/{native,desktop}.rs`、
     `rcu-agent/src/observed_backend.rs`（此前被 trait 默认实现吃掉 → `capability_unavailable`）。

**断点状态**：代码落地、workspace `cargo check --offline` 通过；**尚未跑 fmt/clippy/test
三件套，尚未跑 #3a 活体验收，尚未提交**。

## 3. 用户核心需求（必须持续满足，按时间归纳）

1. **后台独立操作**：RCU 是独立工具，后台操作电脑，**不阻塞/不抢占用户前台**。
2. **显示器硬约束**：只允许在 **MacBook 内建屏** 显示与操作，**绝不跨到扩展屏**；
   不仅窗口移动，**打开应用就应落在指定屏**。
3. **通用 computer use**：不只针对个别应用；非操作应用的其他弹窗也要能靠独立 computer
   use 点击/继续。
4. **模态/弹窗一等公民**：各种对话框、系统面板、非前台弹窗都要能观察与操作。
5. **能力要交付给 agent**：应用有哪些按钮/工具/功能应被列出并交付，由 agent 推断用途，
   而不是只靠一种工具硬套。
6. **视觉反馈缺失**：需要鼠标视觉反馈（AI 光标指示 / pointer feedback）。
7. **应用占用 UI/UX**：类似 codex computer use 的"RCU 正在接管 / 停止"持久提示；用户操作
   被拒绝/介入时的提示。
8. **操作连贯性**：多步操作要连贯，不要每轮停下等确认；context 快满时自动压缩后继续。
9. **必须用 RCU MCP**：绘制/验证要走**已加载的 Host 原生 MCP 入口**，不能偷偷走
   Python/Node/stdio 测试客户端；不能用 shell 桥替代。
10. **参考材料**（用户点名、需纳入参考）：
    - 微信文章《computer use 深度解析》 https://mp.weixin.qq.com/s/3gMB4s_dli1ZjqPpOy3iGQ
    - `https://github.com/EYHN/kwwk-computer-use-core`（可 clone 到 `~/AI Projects` 参考）
    - 邮箱/桌面等为"仿图"任务素材。

## 4. 待办

**#3a 收尾（立即）**：
- [ ] `cargo fmt --all -- --check` / `cargo test --workspace --locked` /
      `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- [ ] #3a 活体验收脚本：绑定 → 观察 → 解析 → 跨屏移动 → 原生回读 → 移回 → 失败码负例；
      强化 no-replay 断言；证据 JSON 落盘（`work/`）
- [ ] 提交（行为变更与架构搬移分开）

**用户点名、尚未完成（高优先级）**：
- [ ] Mac 视觉反馈（AI 光标指示 / pointer feedback）
- [ ] 应用占用 UI/UX 提示（"RCU 正在接管 / 停止"）
- [ ] 系统文件面板跨屏不可搬移 → 需要"系统面板例外/显式豁免"策略（现状 fail-closed）
- [ ] 独立子会话验证机制
- [ ] 正式验收文档与总提交

**既有遗留**（见 implementation-status.md）：语义 observe 全树遍历开销、菜单 focus/pointer
身份、多端故障隔离、资源上界、Linux/移动端按 ADR 分级。

## 5. 原始历史索引

| 文件 | 内容 |
|---|---|
| `work/session-migration-20260915/user-history.md` | 93 条用户消息**全文**（61KB） |
| `work/session-migration-20260915/assistant-milestones.md` | 504 条 assistant 文本全文（248KB） |
| `work/session-migration-20260915/tool-calls-summary.md` | 1012 次工具调用元数据（61KB，无大 payload） |

**迁移原则**：以上存档**仅供按需检索**，默认**不加载**进上下文；日常只需本接续包 +
`implementation-status.md`。
