# AX-less visual-only 部署与 Retina 截图尺寸回归（2026-09-26）

**后续状态**：用户重启后新 Plugin 工具表已加载；又发现生产 Agent `computer.image` 无条件发 view_id 的缺口并已修正/重新部署。详见[生产引用隔离补漏](2026-09-26-production-view-guard.md)。下文 PID、版本和“待 Host 新任务验收”是首次部署时的阶段事实。

接续[非标准 macOS 窗口只读实现](2026-09-25-nonstandard-macos-visual-only.md)。本记录区分离线实现、已分配虚拟屏 Paint S 的 Host 回归、尚缺真实 Wine/GameHub GUI 的边界；没有用 CrossOver 宿主窗或 Arc 的 Steam Support 代替游戏窗口。

## 按时序的实际结果

1. 前版离线门禁 520 passed/10 ignored、fmt/Clippy/diff 通过后，以旧 `RWI Local Development` 证书在 `work/` 隔离副本签署并严格验证；designated requirement 仍为 bundle ID `com.destroyedteam.real-world-interaction.agent` + leaf `24B346A95EC8F450F7D8433A2E1876568DBCB2F8`。只原子替换 RWI Agent.app 并用原 LaunchAgent 重启。Host 重新连上实例 `2396de52-1323-4fe8-97ae-7489f299b8b2` / PID 83906，四项权限仍 true。
2. 对已分配 BetterDisplay 虚拟屏 Paint S 主窗重新 `targets→discover→bind` 后，`snapshot(max_edge=2048)` 仍能读到 2868×1320 原图和 42 个 AX 控件；但 `capture_visual(max_edge=2048)` 返回 **`visual_output_budget`**。同一窗口的 1024/512 档返回成功。没有输入、重试鼠标或更改画稿。
3. 原因是 core visual 校验将 macOS 1434×660 **逻辑点**窗口当成截图原生像素，以“不得上采样”拒绝本来合法的 2048×942 Retina 预览；ScreenCaptureKit 原生来源实为 2868×1320 像素。将 `VisualCapture.source_pixel_size` 设为后端实测的截图原生像素尺寸，输出预算改按它计算；Windows 物理像素源与 fixture 保持相同值。旧 peer 缺字段可解码为未知 `[0,0]`，但新生成结果的校验不接受未知值。加入 Retina 正例、伪上采样负例和旧 wire 解码测试。
4. 最终全仓离线门禁：`cargo fmt --all -- --check`、`cargo test --workspace --locked --offline` **522 passed/10 ignored**、all-targets/all-features Clippy `-D warnings`、`git diff --check` 均通过；release Agent/MCP 构建通过。新 Agent 以同证书签名和同 bundle ID 部署，严格代码签名验证通过；PID 96713 / instance `c5946331-a056-4a5e-b765-f6d8ab9bab68`，macOS 四项权限仍 true。仅替换 RWI Agent 与 client-v1 `rwi-mcp` 二进制，保留 profile、配对证书/私钥、journal 和其他插件配置。已通过官方 cachebuster/reinstall 流程更新 Codex Plugin 至 `0.2.0+codex.20260925164306`；安装版 Skill 含 visual-only 边界。
5. **Host 原生回归**：新实例上重新发现并绑定 Paint `cgwindow:2189`，`capture_visual(max_edge=2048)` 成功返回 2048×942，明确报告 `source_pixel_size=[2868,1320]`、`source_rect=[1536,24,1434,660]`、`ax_walk=false`；紧接着 `snapshot(max_edge=2048)` 仍返回可解析 view 和 2868×1320 原图、42 个控件。工作 `.paint` 仍为 149 对象/484566 bytes，SHA‑256 `8fc26d801ca6940a723e6693406d136ceeb5959a9f2c3cea53b9e8e76f3d3943`，无误改。

## 边界与接续

- 新 Agent 的截图/AX 正常路径已在当前任务真实 RWI Plugin 验证；当前任务可能仍持有重装前的 MCP 进程，**新 MCP 对 AX-less `snapshot.visual_only` 的 Host 验收必须在新任务/Host 重载后、真实目标窗口出现时做**。不以本地二进制存在替代 Host 验收。
- 当前没有位于已分配虚拟屏的 Steam/其他 Wine/GameHub 游戏 GUI。待用户打开并保持窗口可见后，按精确 PID/CGWindow/epoch 重新 discover/bind，只读对比 `capture_visual`、`snapshot.visual_only`、原像素 tile、Live；若 SCK 不可分享或身份变化，应保留错误，不自动换窗口，也不把只读画面升级为输入引用。
- 已保存两份连续 Agent rollback（原旧版及首次 visual-only 版）和旧 MCP 二进制于各自产品安装目录 `rollback/`。未提交/推送；原生 Paint 源和此前阶段快照未覆盖。
