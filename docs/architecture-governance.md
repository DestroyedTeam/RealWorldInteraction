# 工程架构治理与源码归一（2026-09-11）

状态：本地基线已建立、候选已迁回根 workspace；架构分解仍在渐进实施。
首次基线 `afee3d9` 保留原根源码；当前整改分支 `engineering/governance-native-mcp`。
用户随后于本日明确授权 commit/push/DestroyedTeam 远程仓库及提交 AGENTS.md，取代原 Git 禁令。

## 当前证据

- `git branch --show-current` 为 `main`；`git log` 返回尚无任何提交。不是“已有提交但没有合适分支”。
- 根 workspace 仍为 0.3.0 / 5 个 crates；当前候选为 `work/b1-native-link-r2/source`，核心版本 0.4.0-m3a.7 / 7 个 crates。
- `work` 约 69 GB，深度两层有 20 个名为 `source` 的目录。不能把历史快照当作 20 条可维护开发线。
- 当前候选 `rcu-mcp/main.rs` 1635 行、`rcu-native-link/control.rs` 1257 行、`worker.rs` 1143 行；Windows `application.rs` 1627 行、`agent_binding.rs` 1596 行。入口、协议、生命周期和平台证明耦合，单纯增加目录层级不能修复职责边界。
- 数据来自本地当次检查；体积不是待删除清单，源码副本也不代表全部都是可丢弃文件。

## 唯一研发主线

目标：**仓库根 `Cargo.toml + crates/` 是唯一可编辑、构建、审查的产品源码**。
当前 r2 已作为迁移输入归一到根目录，不是已经验收的正式版本。逐文件 SHA 和来源见
`docs/migrations/2026-09-11-source-consolidation.json`；根目录旧源码保留在首次基线中。

迁移顺序：

1. 明确解除本轮禁止 Git 操作的限制后，建立本地基线和整改分支；不 push。
2. 基线提交只纳入经过盘点的源码、锁文件、测试、文档与可复用工具。先审查 staged 文件清单，排除私钥、凭证、测试配置、二进制、IDE/Codex 本地设置、截图和大型原始日志。不能 `git add .`。
3. 将候选差异归一到根 workspace，单独提交迁移；保留根目录旧内容的基线证据，不把候选自动标为验收通过。
4. 后续按“行为修复 / 纯重构 / 测试与接入 / 文档证据”拆分可回滚提交。功能分支开发，经验证后再合入 main。
5. `work/` 仅存可清理缓存、测试临时数据和未归档运行。禁止新增可编辑产品 source 副本。完成迁移后，对历史包建立索引、SHA、保留原因与外部归档位置，再单独确认清理；不得先删除后补证据。

## 模块责任与依赖方向

| 边界 | 负责 | 不负责 |
| --- | --- | --- |
| domain / contracts | 身份、窗口、动作、验证契约、错误分类 | MCP、TLS、OS FFI、文件部署 |
| application / use cases | observe → claim → action → verify，用例与状态机 | UIA/GDI 细节、模型输出格式 |
| safety / policy | 租约、授权、不可重放、取消、实际资源证明 | 模型摘要推断授权、脚本兜底 |
| adapters / windows / macos | 原生捕获、UIA、SendInput、会话与令牌、平台进程生命周期 | 插件打包、MCP 路由、业务目标判断 |
| transport / protocol | MCP stdio、TLS 帧、身份验证与连接生命周期 | 具体应用流程、自动重放 |
| integration / ai hosts | Codex 插件、Skill、模型可读回包 | 改写原始快照、放宽动作校验、伪造 App 注册 |
| deployment / provisioning | 显式 Ordinary/Admin 配置、固定安装根、启动与回收 | 修改其他软件/全局配置、自动批准权限 |

按稳定边界逐步拆分已有 crates，不一次性制造大量空 crate。先切开内部模块与接口，再据依赖和跨平台复用需求拆 crate。`rcu-native-link` 对 Windows 的直接依赖是待隔离平台端口的债务，不因为当前检查允许就视为合理终态。

### 第一批结构变更

- 新的原生插件导出与模型回包投影放在 `rcu-native-link/src/integration/`，明确不拥有原生执行或授权。暂保留 `plugin` / `presentation` 公共重导出，避免把调用方全部改名混入行为修复。
- 下一批拆分 `rcu-mcp`：`main.rs` 只负责启动；独立 `bootstrap`、`tools`、`response`、`worker`、协议测试。先移动职责，不修改工具 schema。
- `rcu-native-link` 后续分为 `protocol`、`transport`、`orchestration`、`deployment`、`integration`；每个模块定义入口与依赖，避免 `mod.rs` 成为新的千行文件。
- Windows 优先拆 `session/token`、`window/uia`、`capture`、`input`、`lifecycle`；FFI 与纯规则测试分离。Ordinary/Admin 共用身份和安装状态，不复制实现绕过 HOLD。

## 可执行约束

`architecture/baseline.json` 记录当前允许的生产依赖和既有大文件上限。
`tools/check_architecture.py` 要求显式 workspace 路径，阻止错把旧根 workspace 当作新候选验收；对新生产依赖和大文件增长报错。
现有豁免是待偿还债务，不是架构通过。默认新文件上限 600 行只是审查触发器，不是通过拆空壳文件达到的质量指标。

工程检查脚本使用 Python 标准库，**仅开发检查，不属于产品后端，也不调用 Windows/MCP**。

## 分开验收

- 结构：唯一源码根、明确依赖、入口职责、可定位模块。
- Git：本地基线、可回滚提交、分支与 staged 审查；不代表已发布。
- 功能：fmt、workspace test、全 target/feature clippy；不弱化原有门禁。
- 接入：真实 Codex 插件与 Skill 发现、实际 MCP 调用；fixture 不等于 Windows 原生验收。
- 性能：回包文本体积、调用次数、服务端耗时、模型端总耗时分别记录。
- 交付：可复现构建、脱敏证据索引、未通过项、版本与 SHA；临时工作目录不再是交付版本来源。
