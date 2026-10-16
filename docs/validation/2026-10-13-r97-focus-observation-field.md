# R97 · 统一切窗成功结果的前台字段（2026-10-13）

## 对象与最小修改

唯一产品源码 `/Volumes/ChatGPT/Real World Interaction`。`NativeMac::focus_exact_window` 的 `foreground_observed` 成功分支新增 `activation.foreground = now`（`crates/rwi-macos/src/native/activation.rs:230`），复用既有 `after_foreground` 同一次原生样本，不追加读取/激活/键鼠事件。`already_foreground` 的 `foreground`、`native_requests:0` 保持；旧before/after、状态、`goal_verified:false`、`input_authority_granted:false`、guard/lease/no-replay全部保持。未把“已在前台”伪装成发生切换。

scope-pristine SHA `45850c054766a89d000a7a110421542d2d55759986b4f8d54c490337ba0a9719` → modified SHA `98eed0db71f240a1bdaecd8fe1280faddb7ef91696060af08ae0b0fb151c5139`。371个源码条目只该文件变化；scope-pristine不是累计全局原始baseline。

## 同输入已执行结果

生产源码中两段实际 `serde_json::json!` 借入纯Rust编译探针，用同一对显式测试身份before PID77/window9、after PID42/window7验证输出。BASELINE实际Cargo exit **1**（切换成功缺统一字段）→ MODIFIED **0**（两成功分支统一字段可读，legacy projection/state/false标志保持）。未调用原生窗口API；这不是设备/权限/前后台接收验收。

本轮真实ROOT候选九门禁均exit **0**：fmt；workspace test；browser example test/build；receiver example test；workspace all-targets/all-features Clippy `-D warnings`；架构基线；架构测试；git diff check。workspace **867 passed / 0 failed / 11 ignored**；browser **12/0**、receiver **89/0**另记。不是借用R95门禁，架构基线通过不等于全部架构完成。

每条实际命令、literal stdout/stderr/exit及source freeze：
`/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/focus-observation-common-field-r97/candidate-transaction-r97/RESULT.json`。

## 回滚与部署边界

沿用同四角色及246条原始baseline；新两报告写前均实测ABSENT。同输入BASELINE/MODIFIED/ROLLBACK/REAPPLIED与恢复hash以本轮独立`cumulative-four-roles-r97/RESULT.json`、固定`VERIFICATION.txt`和final reopen为准，本报告写时不预称未执行的回滚完成。

**Agent/MCP尚未部署此字段，本轮没有GUI输入，不声称Host已加载新字段或后台已修复。** 旧checkpoint check-only冻结为候选前源码，实际exit1 `ROOT source identity changed`保留；改用本轮modified371 freeze独立验证exit0，未重放已完成候选、九门禁或原GUI输入。

## 完整计划继续

R96同Chrome暖态前台counter0→1→2已验；暖态后台2→2且无新DOM鼠标事件，仍负例。default改变activeElement，因此不是cold-private-first单因子结论。键盘归属r96WXYZ→r96WXY→r96WX、12原request终态/释放/no-replay和自有窗口清理沿用，不重复输入。

下一独立路径为默认关闭的public NSEvent构造。v4 marked-copy内存/回归专测通过，但Clippy实际101发现extern签名/unused问题；Win两arch101停在依赖SDK `assert.h`，非候选Rust效果证据。保留实际失败，修候选后再完整门禁/部署/loaded MCP真窗口验收；不偷偷激活、补primer、重发或改默认投递。

Live已验上限280×300pt、两28pt隐藏恢复/关闭控件及任务绑定，源图细节不降。本轮不要求休息用户配合；完整F01–F16、性能、耗时、MCP可读性与跨窗适配不因一个字段修复而封版。
