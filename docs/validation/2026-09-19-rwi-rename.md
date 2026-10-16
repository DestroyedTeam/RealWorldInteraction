# Real World Interaction 统一命名迁移

日期：2026-09-19。用户授权更名 GitHub、源码文件夹、MCP、Skill、项目/任务关联及产品名称。

## 已完成

- GitHub：`DestroyedTeam/RealWorldInteraction`，保持 private；origin 指向新地址。
- 源码目录：`/Users/vincentadamnemessis/AI Projects/Real World Interaction`。原路径为兼容符号链接，无第二份源码。
- MCP：Codex、OMP、OpenCode 三处注册为 `real_world_interaction`，启动新运行目录中的 `rwi-mcp`。
- Skill/Plugin：`real-world-interaction`，三份现有 Skill 来源均已迁移；插件导出器同步新目录名与 manifest。未额外注册重复插件。
- Rust：10 个包、目录、Cargo 依赖、lockfile、二进制及相应测试使用 `rwi-*`。
- JS：`rwi-client.mjs`、`RwiClient` 及相关类更名；worker 的 `rwi` 与旧 `rcu` 是同一个对象，保持租约/输入域一致。
- 已列出的三个 Codex 项目内任务标题带 Real World Interaction 前缀，原任务 ID 与历史保持。
- OMP 项目会话目录更名，旧目录保留链接，原 JSONL 及附件未改写。
- 本机运行目录：`~/Library/Application Support/RealWorldInteraction/`。证书与状态原目录原地移动，旧目录为同一目标的链接；配对未重新生成。

## 验证

- `cargo fmt --all -- --check` 通过。
- `cargo test --workspace --locked --offline`：433 passed，10 ignored。
- `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` 通过。
- `node --test integrations/javascript/rwi-client.test.mjs`：41 passed。
- Skill frontmatter/naming 校验通过。
- 新安装适配器的真实 stdio initialize、26 工具发现、JS 新旧入口同一性及只读设备发现见同目录 `2026-09-19-rwi-rename-smoke.json`。本轮未发送 GUI 输入/截图。

## 尚待完成

1. **Codex 桌面项目显示名**：当前应用缓存会覆盖磁盘元数据。GUI 工具不允许操作 Codex；CLI 的项目接口查不到桌面端的旧项目 ID。三个任务标题已经改名，但当前项目列表仍显示旧名。退出 Codex 后，在终端执行下列脚本，再重新打开核验：

   ```sh
   rtk python3 '/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/rename-20260919/finish-codex-project-rename.py'
   ```

   脚本在 Codex 运行时拒绝修改；保留原项目 ID、任务历史及原状态备份，只更新项目名和根路径引用。

2. **本机原生 agent 运行进程**：新 `rwi-agent` 已编译并暂存到安装目录；当前 agent 没有重启。新光标字形/标题尚未在实时 GUI 中验收。
3. **Windows 部署**：设备连接超时，未唤醒 VM，未迁移其安装目录/任务名称。源码仍保留固定旧安装锁根与命名管道作为兼容身份；不得另起新输入域。
4. **历史业务待办**：Paint S v14 头像独立图层尚未执行。v13 是继承的历史报告，不是本轮验收。

## 兼容边界

证书 DNS `rcu-agent.local`、TLS ALPN、wire schema、既有环境变量、持久状态/锁路径及历史证据继续保留。它们参与跨版本信任、互斥与防重放，不以字符串替换改变。旧部署与历史脚本可经兼容目录访问原文件；后续逐端部署再收敛进程名称。
