# R94 · MCP snapshot 默认重复元数据精简（2026-10-13）

## 已实现的最小变化

仅改变 Rust MCP 适配器默认成功 snapshot 的展示投影：`compact_snapshot_diagnostics` 在完整原件进入结果缓存后，只有 `coverage` 与 `consistency.ax_coverage_after` **精确相等**时，将后者替换为 `consistency.ax_coverage_after_ref: "/coverage"`。

窗口／PID、完整元素 ID、名称、actions、point、frame／view／坐标映射、coverage 的 false/null/reason/限制、未知字段、输入可用性和所有图片均保持。没有短 ID、原生输入、焦点变化、生产坐标修改或自动 fallback。不是删除未知诊断的字段白名单。

形状未知、缺少非空缓存引用、已有 ref、coverage 不同、错误结果及 `include_timings=true` 不执行去重。可选 `operation` 若不完整／非终态／未释放／仍 pending／释放不确定，保留 inline coverage；**原有默认 timing 删除规则仍可能适用**，不宣称任意不确定 snapshot 的整个原件原样展示。

`computer.result(details_ref)` 保留完整原件及其 metadata SHA；读取缓存不会再做 native RPC、截图或重放输入。

## 实际工程验收及保留的失败

- 最初六项相同输入测试 BASELINE exit101（实际断言失败，而非编译失败）→ MODIFIED exit0。
- 首全 feature Clippy 实际 exit101：`nonminimal_bool`。未禁用检查，改为等价 `is_none_or`；修正后独立命令 exit0。历史错误 stdout/stderr 保留。
- 增加可选 operation 的保守保护后，真正执行新 Clippy、fmt check、七项测试，三项 exit0。测试 stdout：`7 passed; 0 failed; 0 ignored`。
- 同四角色的独立副本匹配 BASELINE／MODIFIED／ROLLBACK／REAPPLIED、全部工作区门禁和后续真实 MCP 进程验收，以追加证据记录为准；这些不是上述专测自动代替的验收。

## 字节证据与界限

R92 已记录六份真实 Chrome snapshot 的**序列化反事实模型**：重复体479B，显式 pointer36B，净443B，median约5.322%。保留原 text8414–8558B与重新序列化8320–8464B的差别。这里不是已测 token、native CPU、采集延迟、Host 等待或总体成功率提升。

下一项实际新 MCP 格式进程测试采用临时 mTLS Rust mock device 与明确合成场景；原六份投影结果不是原始 DeviceResult，不把它们反向拼成“原生日志重放”。须分开记录 content-text、wire、structuredContent、image/base64 和 cache 原件恢复。

生产 Agent 仍沿用 R92 的签名版本及四项权限。此报告写入时 MCP 新 formatter 尚未部署，当前 Host 也仍旧 inode；后续 MCP-only 原子部署不应重启 Agent、其他会话 adapter 或要求用户重启。

## 保留的完整计划

R92 AppKit 真后台单击和前台键盘接收权已有独立 goal；Chrome 默认／private 单对 click counter仍0→0。R93仅合成 window 的 CGEventCreateCopy 内存校准，不能解释 Chrome 接收失败，未改生产点。

继续补真实客户端 document down/up/move 观察、冷后台 hover、真正后台键盘、不同窗口状态、生命周期和性能层级；短 ID／增量树另按严格 frame/target/cache/no-replay 契约推进。未宣称 F01–F16 完整封版。

Live 沿用已验上限280×300pt（原实际280×269pt）、两枚28pt隐藏／恢复与关闭控件；不降源画质，不重新声称本轮用户亲眼看到。用户休息期间不请求人工切换、展开、重启、密码或授权。

## 证据入口

- 实际执行：`work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/mcp-snapshot-readability-proposal-r94/actual-run-r94/RESULT.json`
- 新保护的真实执行：同目录 `OPTIONAL_GUARD_RESUME_DISPATCH_R94.command_execution.json`
- 最初失败和等价修复：同目录 `actual-run-r94/MCP_ALL_FEATURE_CLIPPY.command_execution.json` 与 `ROOT_REPAIRED_CLIPPY_R94.command_execution.json`
- 六场景字节模型：同目录 `READ_ONLY_PROPOSAL_R94.json`（历史 prepared-only 状态不改写成实际执行证据）。

## 同四角色（累计原基线不替换）

1. `/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/MODIFIED_FILE`
2. `/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/DIFF_FILE.patch`
3. `/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/VERIFICATION.txt`
4. `/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/ROLLBACK.sh`

只对标记的独立验证副本运行可执行 rollback，不回滚生产、不重放 GUI；绝对路径、原 hash／ABSENT 恢复与 literal 输出以最新追加 ledger 和重新打开结果为准。

## 最新追加：真实格式、回滚与 MCP-only 部署已完成

此前“待验收／未部署”描述为初次报告写入状态；本节按随后实际事件更新。

1. **七项同输入 Rust 验证副本**：BASELINE `4 passed / 3 failed`、exit101 → MODIFIED `7 passed / 0 failed`、exit0 → 可执行 ROLLBACK 恢复两份 R93 原 hash，再叠相同测试 oracle，`4 passed / 3 failed`、exit101 → REAPPLIED `7 passed / 0 failed`、exit0。累计原238基线保留、当时扩展240路径；config同输入1/0/1/0、所有原 hash／ABSENT 恢复。首次副本编译遗漏 `bootstrap.js` 实际101保留；是冻结副本依赖闭包错误，不是功能 BASELINE。单独补未修改 support 哈希后只续第一未完成编译，已完成项不重播。
2. **实际生产 MCP 可执行文件＋Rust mTLS mock device**：六个明确合成场景，各测默认／timings，旧 `5204c6…` 与新 `fe13586…` 两进程均exit0；24次 Observe／Image prepare，不接生产 Agent、不发送输入。每默认 content text8393–8398B→7893–7898B、净500B；structuredContent编码也各净500B，JSON-RPC line各净1040B。固定PNG恰好一张、MIME和原文完全一致；全缓存分片导出、SHA及原始JSON精确恢复，查询缓存不增加device RPC。此500B属于该合成场景真实协议格式，区别于旧场景443B反事实模型；没有 native像素、CPU、latency、token或Host效果结论。
3. **常量新格式期待的二进制同输入证明**：独立固定Rust harness、`EXPECT_COMPACT=1`不变，仅目标binarycopy变更。BASELINE101 → MODIFIED0 → sibling pristine可执行 ROLLBACK101、旧binary SHA `5204c6d3734e8c094b7f164ff4e54de3d089f4708056e6a109dd7a216f3b3af9` 精确恢复 → REAPPLIED0。旧EXPECT0的六场景兼容接受是另一个exit0探针，不把早失败的EXPECT1冒称旧六场景全部跑完。未回滚生产或重播七项已完成unit proof。
4. **新完整九门禁**：包含新增Rust集成测试和JSON fixture的371冻结输入前后相同；fmt、workspace、all-target/all-feature Clippy及余六门禁实际全0，**867 passed / 0 failed / 11 ignored**。旧866/0/11九门禁与两次wrapper whitelist准备错误保留，不覆盖成新scope结果。
5. **MCP-only 原子部署已实际exit0**：旧installed probe0／candidate0／atomic replace0／newinstalled probe0，安装SHA `fe13586a68fbb02915f7738f9ccd0fed601883589d8c22ad0eb8fa9ffb09168e`。恰好42唯一工具、所有expanded input schemas与typed baseline相同；四权限有效、domain未fenced、Agent88136／instance d9ecca19不变。11项配置／profile／公开证书／Agent／插件受保护文件hash不变。未替换证书、不读出或导出私钥、不重启Agent／Codex／其他adapter。**当前Host仍旧inode／schema／formatter，不冒称本会话已刷新**；新stdio过程及安装文件通过，不要求休息中的用户重启。

实际入口（完整stdout/stderr/status和补充角色以同一ledger为准）：

- `/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/mcp-snapshot-readability-proposal-r94/cumulative-four-roles-r94/RESULT.json`
- `/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/mcp-snapshot-readability-proposal-r94/format-binary-copy-proof-r94/RESULT.json`
- `/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/mcp-snapshot-readability-proposal-r94/operator-preparation/actual-real-mcp-format-r94/ACCEPTANCE.json`
- `/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/mcp-snapshot-readability-proposal-r94/operator-preparation/installed-mcp-r94/DEPLOYED_MCP_ONLY_R94.json`

R92真实Chrome后台消费缺口、R93合成内存校准的范围，以及完整F01–F16剩余计划全部保留；不把输出去重当成后台问题修好。下一项继续真实document down/up/move接收对照，不重新播放原请求。
