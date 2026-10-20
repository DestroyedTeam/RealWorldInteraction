# 2026-09-11 工程治理与原生 MCP 接入验收

代码验证点：`a46d4cd`。范围是本次工程整改，不是产品全量/native release 验收。
原始日志和大型产物不提交；相对位置、大小、SHA 见同名 JSON 索引。

## 已完成

- 从无提交的根目录建立本地历史基线 `afee3d9`，在整改分支归一根 workspace。
- 95 个候选源文件/锁文件/资产逐文件记录迁移来源与 SHA；没有删除历史冻结快照。
- MCP `main.rs` 1635 → 13 行；启动、参数、服务处理、原始单测分离。
- Native-link 分为 protocol / transport / orchestration / deployment / integration，并保留稳定公共门面。
- 新增可执行依赖/大文件债务约束、Git 源码卫生检查和 macOS/Windows CI 定义。
- Rust 原生插件导出器生成真实 MCP 入口与 Skill，验证原 entry 与 relay image；不复制密钥、不全局安装、不伪造 App ID。
- 在隔离 `HOME/CODEX_HOME` 下通过真实 Codex 安装插件、发现 Skill，并经 Rust TLS 调用 `computer.targets` 和 `computer.observe`。worker 明确为 fixture，没有调用模型、没有控制 Windows 桌面。
- Rust compact 回包先保存完整原始 RPC，再做模型视图投影；原始错误、执行回执、目标验证、图片和额外图片映射保持可用。

## 测试

| Gate | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 通过 |
| `cargo test --workspace --locked --offline` | 223 passed / 0 failed / 1 ignored |
| 全 feature workspace tests，显式 fixture worker | 265 passed / 0 failed / 2 ignored |
| 实际 Codex Host opt-in 测试 | 1 passed，单独显式执行，未假装默认执行 |
| Host 全 target / feature clippy `-D warnings` | 通过 |
| Windows ARM64 全 target / feature 交叉 clippy | 通过；不是 Windows runtime 验收 |
| 架构检查 + 4 项检查器测试 | 通过；既有债务不因此消失 |
| 实际导出插件及 Skill 静态校验 | 通过 |
| 完整受控原始回包的 SHA 读回校验 | 通过 |

`crates/rcu-mcp/tests/stdio.rs` 与冻结 r1 完全相同，SHA 为
`3a982409ccfa4559dcd7f260112a5c019200a256a73ad924619eefdaba7ee1e3`。
没有放宽原始 EOF 的 1 秒门禁。

## 体积测量（离线重放保存的 Windows 回包）

| 回包 | 原文本 bytes | 投影后文本 bytes | 变化 |
| --- | ---: | ---: | ---: |
| response-006 observation | 136876 | 16925 | 减少 87.63% |
| response-018 observation | 76459 | 18823 | 减少 75.38% |
| response-198 error | 569 | 863 | 增加 294 bytes，保留原错误并补齐审计引用 |

不是所有回包都变小，也没有用这个结果宣称模型理解或总体任务耗时已改善。
PNG 保持原样，文本体积和完整 wire 体积分别记录。

## 发现并保留的失败

1. 首次迁移漏了 `fixtures/instructions.txt`，根编译失败；补入资产并更新迁移清单。
2. MCP 模块抽取后缺少两个作用域导入，编译失败；补齐，不改测试断言。
3. 移动 worker 测试模块后，子测试过滤器仍写旧模块名，实际未执行 sleep fixture，强制回收断言失败；改用真实 `module_path!`，原时间界限和 forced 断言保留。
4. 并行网络测试 hard-link 同一 relay inode，其他 case 改变 ctime 导致合法的 image-change 拒绝；每例改为独立副本并断言 link count 为 1。没有放宽生产端文件身份检查。
5. 回包投影审查发现额外文本可能含 downscaled image 的 view mapping；新增保留逻辑及回归，不能只按 `type=text` 一概丢弃。

旧失败日志保留在忽略的 `work/native-mcp-remediation/`，不是用新结果覆盖旧记录。

## 仍未完成

完整 Windows TLS 业务验收、Ordinary/Admin 部署接线、正式用户当前任务安装、真实模型耗时/理解评估、注册 App、剩余大文件的职责拆分，以及经过独立盘点确认后的历史缓存归档/清理。
GitHub CI 的结果应以对应远端 run 为准，不把 workflow 文件存在当成通过。
