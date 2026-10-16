# 新任务加载后：生产 image 引用隔离补漏（2026-09-26）

接续[AX-less visual-only 实现](2026-09-25-nonstandard-macos-visual-only.md)与[首次部署/Retina 修复](2026-09-26-visual-only-deployment-retina.md)。用户重启 Codex 后，本任务加载新版 Plugin Skill/工具描述，`computer.targets(device_id=mac-host)` 在新 MCP 上返回 macOS Agent `c5946331-a056-4a5e-b765-f6d8ab9bab68`、PID 96713、四项权限 true，声明 `visual_only_snapshot.implemented=true`。当前新任务 target/context 与重启前不同，重新发现绑定虚拟屏 Paint S，正常 AX 快照和 2048px 原像素图可读。

## 发现与修复

1. 新 Host `computer.snapshot` 对正常 Paint 主窗返回 `visual_only=false`、view_id 和 61 个 AX 控件，但 `input_reference_available=null`。直接 `computer.image` 同样有 view_id/resolve_tool，却没有该布尔字段。代码复核发现先前的 visual-only `view_id` 阻断只落在**本地 Server** 的 `make_view`；生产 Plugin 实际为 `Gateway → device RPC → rwi-agent Executor::image`，后者仍无条件登记图像 view。对 AX-less 场景，底层 `resolve` 仍会拒绝无 Binding，但向 Host 签发可解析 view_id 的展示契约是错误的。
2. 在**生产 Agent `Executor::image`**改为只有 scene 同时有 native Binding 和 `pixel_to_desktop` 才登记 view；visual-only 的 PNG/native tile 照常返回，但 `view_id=null`、`resolve_tool=null`、`input_reference_available=false`，`resolve_view` 为 `unknown_view`。Gateway `snapshot` 同时按原场景 Binding/像素映射裁决 Host 公开的 view_id，连旧 Agent 错发的 ID 也不转成快照输入引用。无图 semantic-only 显式报 `false`，正常 AX 路径保留 `true`。
3. 离线回归直接走生产 `Executor::invoke(computer.image/resolve_view)`：视觉层 image block 和8×4原像素 tile 可读，无 view 登记，解析拒绝；AX 有绑定路径仍给 view。Gateway 单测覆盖旧 Agent 错发 ID 的防御、semantic-only 无映射及正常兼容。最终全仓 `cargo fmt --all -- --check`、`cargo test --workspace --locked --offline` **524 passed/10 ignored**、all-target/all-feature Clippy `-D warnings`、`git diff --check`、release Agent/MCP 构建通过。
4. 用同一 `RWI Local Development` leaf `24B346A95EC8F450F7D8433A2E1876568DBCB2F8` 重新签名并严格验证，只替换 RWI Agent 与用户级 client-v1 MCP，原 profile/密钥/journal/其他插件不变。新 Agent PID 52564、instance `8d23148e-c118-4ab3-a0ce-27951f8ca6b1`，四项 macOS 权限仍 true。Codex Plugin 按官方 cachebuster/reinstall 更新到 `0.2.0+codex.20260925172749`，未手改 marketplace。备份在各产品安装目录 `rollback/`。
5. 当前任务真实 RWI Host 对虚拟屏 Paint 主窗重新 `targets→discover→bind→snapshot/image`：`visual_only=false`、`input_reference_available=true`、普通 view/controls 均可用；`capture_visual(max_edge=2048)` 仍成功 2048×942、原生源2868×1320、`ax_walk=false`。这证明正常分支未回归；visual-only 分支只有离线生产 Executor/Gateway 回归，**尚无真实 Wine/GameHub GUI 来做 Host 实机验证**。重装后当前任务是否换到新 MCP 进程不能仅凭磁盘版本推断；待真实靶窗时再核对。

当前只读 Computer Use 应用清单显示 CrossOver、GameHub、Wine Steam、奇兽杂货铺均未运行；不能把 Arc 的 Steam Support 或 CrossOver 宿主窗当作游戏表面。未提交/推送。
