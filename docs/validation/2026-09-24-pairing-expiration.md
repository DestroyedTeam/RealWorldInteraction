# mac-host 配对过期与恢复（2026-09-24）

## 故障事实

- 当前任务的 `computer.targets` 返回 `device_connection_failed`，TLS 原因是对端证书到期；证书到期日为 2026-09-21 01:28:30 UTC。
- RWI Agent LaunchAgent 仍运行且监听 `127.0.0.1:45129`，并非进程未启动。
- 仓库 `rwi-link-trust::provision` 当时固定生成 3 天 leaf、7 天 CA。CA 私钥不会持久化，无法在原配对上原位续签。
- Windows VM 仍为独立的 `device_connect_timeout`，未更改其配置或尝试唤醒。

## 修正与部署

- Rust provisioning 的私有 CA 改为 365 天，agent/controller/model leaf 改为 180 天。TLS 完整链、时间和固定 pin 校验保持原样，没有放宽验证。
- `provision_cli` 加入精确有效期回归。`cargo fmt --all -- --check`、`cargo test --workspace --locked --offline`（467 passed、10 ignored）、`cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` 及 release provisioning 构建通过。
- 新配对置于 User/Host 安装根目录 `agent-mac-v1/pairing-20260924/`。原 `pairing-20260917/` 和原配置备份仍保留在安装根目录，不进入 Git。新配对公开元数据见 [public manifest](2026-09-24-pairing-public-manifest.json)。
- 原子更新 RWI 自身 Agent 配置与 mac-host 客户端 profile，Windows 设备条目、端点、Agent 持久 journal 路径保持原值。仅重启 RWI Agent LaunchAgent，没有重新签名 APP 或改变 TCC 授权。
- 证书 readback 和 pin 交叉检查通过：Agent/model/controller leaf 均到 2027-03-23 03:58:59 UTC，CA 到 2027-09-24 03:58:59 UTC。

## 通信验收与当前限制

- 新启动的**实际 MCP 子进程**读取生产 Host profile，`initialize` 成功，`computer.targets` 返回 mac-host、Agent healthy、四项 macOS 权限 true；这是生产传输及配对验收。
- 本任务已加载的旧 MCP 进程仍持有旧内存中的 TLS 身份/pin，返回 `UnknownIssuer`。只终止本任务启动于 2026-09-24 11:50 的 RWI MCP 进程后，Codex 工具仍返回 `Transport closed`；没有继续终止另一条旧任务的进程。
- 因此本任务**尚未通过重新加载后的 Host 原生 GUI 验收**。需重启 Codex 或开启已更新工具目录的新任务，重新调用 `computer.targets`，再按 macOS Skill 发现目标/窗口/显示器。不应把独立子进程的成功冒充为当前任务可操作。
- 系统显示器只读清单确认内建屏幕及无线 Sidecar `OpenDisplay — VincentAdamNemessis's iPhone` 在线、扩展模式，逻辑尺寸 1434×660。尚未移动或操作 Paint S；重连后必须以新布局读取映射。

## 原因与长期安排

180 天有效期降低开发周期中的突然过期概率，但仍会到期。未来需在到期前实现完整的配对轮换与 Host profile 热加载，或提供明确的健康检查/重连流程；不可关闭 TLS 时间校验。证书时间边界由 [RFC 5280](https://www.rfc-editor.org/rfc/rfc5280#section-4.1.2.5) 定义。
