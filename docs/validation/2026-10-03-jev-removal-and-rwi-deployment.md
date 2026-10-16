# Jev 清理与 RWI 签名部署

日期：2026-10-03。按用户明确请求，移除 Codex 中的 Jev MCP/Skill 集成，信任既有本地 RWI 代码签名证书并部署 Release Agent/MCP。只处理 RWI 与 Jev 相关状态，不改其他 Codex/MCP 配置。

## Jev 移除

- 从活动 `~/.codex/config.toml` 删除 `[mcp_servers.jev]`（`https://www.jevai.org/api/mcp`）段；TOML 解析通过，检查活动替代 `*.toml` 配置无其他 Jev MCP 注册。
- 删除仅供该 MCP 取 Keychain bearer token 的 `~/.codex/bin/jev-mcp-headers.py`，并删除精确匹配服务/账户 `codex-mcp-jev` / `JEV_API_KEY` 的登录钥匙串通用密码项；没有读取或记录密钥值。
- `mcp__codex_app__uninstall_plugin("Jev")` 返回 `not_installed`；用户 skills 目录及插件 cache 中没有独立 `jev*` skill/plugin 文件。因此活动 config 是找到的 MCP 注册点。当前 Codex 进程仍缓存启动时工具/技能定义；需要重启后才能确认它们不再出现在新 task。

## 证书信任与部署

- 使用现有 self-signed `RWI Local Development` 证书及其留在 login Keychain 的 RSA-2048 private key；SHA-1 leaf `24B346A95EC8F450F7D8433A2E1876568DBCB2F8`，有效期至 2036-09-16。没有创建/导出/更换私钥。
- 按用户请求，仅在当前用户 trust settings 中增加 `Code Signing` policy 项；未指定 `-d`，未更改 System/Admin trust、TLS 或其他 policy。`security dump-trust-settings` 显示 Proxyman CA 原状态不变、RWI 仅 1 条 Code Signing trust setting；`security find-identity -v -p codesigning login.keychain-db` 返回 1 个有效 identity。
- 从现有签名 Agent.app 暂存新 Release `rwi-agent`，使用同一 leaf 签名；`codesign --verify --strict --verbose=2` 通过，Designated Requirement 仍为 `identifier "com.destroyedteam.real-world-interaction.agent" and certificate leaf = H"24b346a95ec8f450f7d8433a2e1876568dbcb2f8"`。安装 executable SHA-256：`660730276b012336b2c66291a345fcab61c273c6fcdc71e6f8f01c83dcddff96`。
- 更新已配置的 MCP binary 路径；安装 SHA-256：`b478b93561a97bc31c100478ace4302ae8652c48ede911c6d93aede9e7cd2f76`。没有改 `.mcp.json`、profile、TCC 或 LaunchAgent plist。
- LaunchAgent 已 stop/swap/bootstrap；新 Agent PID **976**，launchd `state=running`，`last exit code=(never exited)`。Agent/MCP 原件回滚备份在 `~/Library/Application Support/RealWorldInteraction/rollback/20261002-rwi-deploy-01/`。系统权限没有重置；因当前 task 没有 RWI MCP 工具，新的 Host 工具目录与权限仍待重启后的实机复验。

## 尚待验证

Codex GUI 未重启。`cua.getApp("Codex")` 被当前 Computer Use 明确拒绝访问，`mcp__codex_app` 没有应用重启接口；没有用 shell 强杀宿主以绕过此限制。当前旧 RWI MCP 子进程/工具表可能仍驻留，换上的 adapter 将由下次 Codex 启动加载。请重启 Codex 后回到此 task；后续核验 Jev tools/skills 已消失、RWI 新 tools/list 已加载，再实测 Agent 权限和 selected-window workflow。

Release 候选此前已通过 workspace locked/offline 576 passed / 10 ignored、fmt、Clippy 和 release build；本次只做签名、安装与服务重启，没有改产品源码。RWI certificate preference 与 trust scope 已分别记录在用户要求的 scoped memory 文件中。

## 2026-10-03 Codex重启后复验

- 新会话目录实际加载38个 `mcp__real_world_interaction__*` tools，没有 Jev MCP tool；RWI 插件已启用。上一节关于等待重启的工具目录状态由本节结果更新。活动配置无 Jev MCP stanza，专属 header helper 不存在。
- `computer.targets` 选择 `mac-host`（macOS）：服务 `healthy/available`、Agent PID 976；Accessibility、Listen Event、Post Event、Screen Capture 权限均为 true，`requests_permission=false`。Windows VM 返回 `device_connect_timeout`。当前 Agent capability 显示 attach/detach、discover、snapshot、typed scroll、Live 可用；lifecycle、launch、activate 未实现。
- Agent strict codesign verification 通过；安装 Agent/MCP SHA-256 分别为 `660730276b012336b2c66291a345fcab61c273c6fcdc71e6f8f01c83dcddff96` 和 `b478b93561a97bc31c100478ace4302ae8652c48ede911c6d93aede9e7cd2f76`，与部署记录一致；LaunchAgent 状态 `running`。
- 只读 `computer.discover` 返回首批8/22个窗口及两块显示器 UUID；回执为 `activated=false`、`native_dispatch_entered=false`、`input_sent=false`、`resources_released=true`。未调用截图、窗口绑定或输入工具。游标在10秒目录缓存期限后返回 `catalog_expired`；应重新 discover 后立即分页。窗口级截图/AX、多窗口、Live 和原计划其余验收仍未完成，需明确分配具体窗口后继续。
