# F16：targets typed opt-in 确切文本去重（2026-10-13）

## 实际结果

3个ROOT MCP源已修改：computer.targets可选 string enum `text_projection="exact_duplicates_v1"`；省略默认legacy text JSON/structured输出不变，未知/null/bool/object在RPC前拒绝。仅text删除同设备 `service_process_status` 与 `runtime_status` 的确切重复，`presentation.text_projection.exact_duplicates` 为“被删JSON Pointer→同一文本canonical Pointer”；完整structuredContent与immutable raw metadata cache/hash保留，未知字段、ID、权限false/null、坐标、能力及错误不丢。未匹配或未留存shape inline回退。

固定Rust callercompile0；两fresh release MCPbuild0；同input `RWI_F16_EXPECT_OPT_IN=1` 的基线业务101（旧schema不支持、清理后最终断言）／新实现0；7focused tests、全9ROOT gates实际0，source374冻结。这不是missingtest/compiler失败。两phase均只访问独立fake mTLS设备Describe/OpenContext/CloseContext，server/profile/MCP实际清理，生产连接/输入/截图0。

| 合成实际MCP样本 | 默认text UTF-8 B | 显式opt-in text B | 默认→opt-in JSON-RPC line B | structured/cache |
|---|---:|---:|---:|---|
| exact duplicated status | 10588 | 6234 | 26319→19539 | 原值完整 |
| status unequal | 10589 | 10589 | 无压缩；request-id位数可能使wire差1B | 原值完整 |

字节不等于token；不推断native CPU、capture、Host队列或真实工具wall-time收益。仅格式层接受，不宣称模型信任/GPT理解度已提高。当前生产MCP仍fe135、当前Host新schema未验证，新MCP未部署。

## 证据与尚待动作

- [ROOT真实RESULT](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/mcp-targets-output-audit-f16/typed-opt-in-proposal/candidate-transaction-f16/RESULT.json)
- [全9真实command ledger](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/mcp-targets-output-audit-f16/typed-opt-in-proposal/candidate-transaction-f16/OFFLINE_FINAL_GATES_F16.json)
- [新实现格式+清理](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/mcp-targets-output-audit-f16/typed-opt-in-proposal/candidate-transaction-f16/MODIFIED_SAMEINPUT_FORMAT.observed.json)
- [独立代码审查](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/mcp-targets-output-audit-f16/typed-opt-in-proposal/READ_ONLY_CODE_REVIEW_F16_ACTIVE_CANDIDATE.md)

259全球baseline/ABSENT继续保留；本报告与outputs REPORT原ABSENT已先记录。后续同四角色261的实际source-copy MCP compile0×4／同oracle runtime101/0/101/0及默认原哈希/ABSENT恢复重应用待parent执行，不在本段预称完成。

同批Live44只补已部署native有限范围：8hide外围、gap/cancel、close一点、原PNG约28pt，AX44矩形未实测。画面保留[Live原始报告](/Volumes/ChatGPT/Real World Interaction/outputs/validation/live44-compact-native-controls-2026-10-13/REPORT.md)，不伪造MCP GUI截图。Chrome后台缺口与全F01–F16继续，休息用户无需参与。


## 后续实际闭合：F16四角色与MCP-only部署（2026-10-13）

前文“未部署／四相待执行”保留为当时状态；现同四角色261已实际完成：默认配置1／0／1／0，真实完整源码副本MCP各相编译0、同输入runtime101／0／101／0；原259基线及新增ABSENT恢复后重应用，未重放已完成ROOT门禁或原生输入。

- 生产MCP已从fe135…原子替换为c778a450…；旧安装、候选、安装后三个fresh独立stdio配对smoke全部exit0，均42项schema准确匹配对应冻结记录。候选与新安装的显式targets opt-in真实Describe成功；默认text／structured保留。新增schema仅targets可选text_projection及已部署Agent支持的独立AppKit click delivery变体。
- 只替换MCP程序，Agent仍61970／e52390d8／a54267d4…，profile439c46c9…及其他11项保护文件不变。四项权限true、requests_permission=false；smoke不输入、不截图、不请求OS权限。未重启Agent、Codex／Host或其他适配器。
- 当前Host仍保留旧映射进程／schema，未自动刷新；fresh独立MCP成功不等于新模型会话已验收，newModelReady=false。三次smoke实际callback均0；明确拒绝callback的处理器仅静态核对，本次没有拒绝运行验收。字节证据不转称token、native性能或模型信任提升。
- 两次可选checksum字段KeyError发生在项目runner启动前，保留原tool output／code1，不编造分离stdio；随后首次真实runner guard exit1混淆manifest文件SHA9df5…与规范映射SHA49c57…，发生在smoke／安装前。已分开严格绑定两种SHA、保留完整源与编译provenance检查，修正后的真实安装exit0；原错误及旧operator均保留。
- Chrome后台接收尚未修复。下一步仍为独立冷后台Chrome首pair→前台独立校准、F05冷entry／exit及其余键盘、状态、foreign层叠、payload／DnD、取消恢复、内容route；不重复已验收R99／R103／R104，也不要求休息中的用户配合。

[实际安装与独立审计](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/mcp-targets-output-audit-f16/typed-opt-in-proposal/mcp-deployment/READ_ONLY_ACTUAL_MCP_ONLY_DEPLOY_AUDIT_F16.json)；[安装后的12项保护哈希](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/mcp-targets-output-audit-f16/typed-opt-in-proposal/mcp-deployment/PROTECTED_POSTDEPLOY12_F16.json)。
