# R91：真实浏览器多窗口兼容验收（2026-10-13）

当前：Real World Interaction / 前台 Chrome 修复已部署且实机通过，真正后台仍未达标 / 下一项后台接收端实验。

## 范围与结论

唯一产品源：`/Volumes/ChatGPT/Real World Interaction`。全部验收键鼠来自本任务**当前加载的 RWI MCP**；Rust 回环网页只记录实际 DOM 事件及值，不执行 `.click()`、`.focus()`、CDP 或合成 DOM 输入。另开 disposable Chrome profile，没有操作用户已有 Chrome907、Arc896、游戏、飞书、文档；它们的进程出生时间保持不变。用户无需切窗、展开、重启或授权。

- 自有 Chrome155.0.8059.39／PID68778／出生标识68778:1791830550:652971，普通A46546、普通B46555、隐私C46561同一PID；原生Incognito badge及AX标题独立确认C，不依据页面自称“private”。窗口在虚拟屏4（UUID13E4E9B0-47F2-4043-9CDC-878D5441BD41）。Chrome忽略B/C启动位置，三窗口重叠；没有隐藏的搬窗脚本。
- **通过**：A/B/C分别点击计数0→1、各自文本独立；A Unicode与Left光标、C滚动、C精确关闭、旧C引用拒绝、同PID被遮挡A拒绝。
- **未通过**：Codex前台时A后台计数1→1、请求`-BG`后值仍`R91-A-你好`。至少可信keydown G确实到DOM（code=KeyA，修饰键全部false，documentFocused=false），但没有完成编辑。不能称“所有事件未送达”，也不能据一个G断言整段字符/键码协议正确。事件返回不是业务成功。
- 前台PID/window元组未被后台两请求改变；**未证明**原前台窗口的完整键盘接收权。这不是全后台完成、F14/CDP集成或所有应用兼容认证。

## 问题与最小修复

旧版同一点AX命中从Button变成该Button的StaticText标签，纯CFEqual误当遮挡。第一部署的严格重采样仍拒绝`ax_hit_not_stable`（counter0），完整保留，未算通过。

最终修改六文件中的精确分支：`observed_element`、`NativeMac::metadata_read`、`settle`、`settled_hit::observed`、`NativeMac::input_anchor_facts_in_sequence`、`label_hit::equivalent/label_ancestor/bounded_read`。只接受**原AXButton的非交互AXStaticText后代**：同provider/window/geometry、真实CFEqual祖先链最多8层；交互/未知action、cycle或跨provider/window则拒绝（空action以及显式允许的AXShowMenu/AXScrollToVisible不误当交互）。canonical仍是原retained Button，不按文字搜索或换兄弟对象；只扩展显式element Pointer/DoubleClick，pixel/text/keyboard和其他安全门禁未放宽。

精确操作重验证最多3样本、连续2次等价；每proxy配置有限超时并读前后检查，应用与对象超时恢复。120ms／label40ms为**软验收预算**，不假称系统调用可硬抢占。全snapshot仍单遍控制树，不每个节点等120ms。

Chromium公开实现的async hit-test说明支持这项诊断，但不能把HEAD源码冒充本机155的精确编译版本。一次源码URL读取失败已保留，既有原文/hash证据见 `/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/browser-window-lifecycle-r91/PRIMARY_CHROMIUM_AX_HIT_EVIDENCE_R91.json`；参考[Chromium Cocoa实现](https://chromium.googlesource.com/chromium/src/+/HEAD/ui/accessibility/platform/browser_accessibility_cocoa.mm)、[manager实现](https://chromium.googlesource.com/chromium/src/+/main/ui/accessibility/platform/browser_accessibility_manager.cc)。

## 按实际顺序验收

|步骤|实际结果|限制／问题|
|---|---|---|
|旧版可见A/C点击|stale_reference，dispatch前拒绝，counter0|误拒绝已复现；covered A拒绝仍正确|
|第一候选strict settle|Button→StaticText，ax_hit_not_stable，counter0|只用于诊断，不报修复成功|
|最终A|counter0→1；文本R91-A-你好；Left caret8→7|DOM、AX及原图回读；B/C值未变|
|最终B|counter0→1；文本R91-B|B前台时A被遮挡；其文本请求visibility_unknown且零dispatch，不是独立focus-only负例|
|最终隐私C|counter0→1；文本R91-C；scrollTop0→240|滚动使用可见内AXGroup；外Group中心被后代遮挡未强点|
|关闭C|具名AXPress仅关C；A/B保留|第一CG枚举短暂幽灵行；之后稳定缺席，旧snapshot/input均window_missing，不重定向|
|实际OS Command+Tab|RWI全局工具4事件/配对释放；后续native读到Codex9697|receipt内部after尚为Chrome，OS队列异步；不是物理人工来源|
|后台A两请求|click/type均returned，counter1、文本未变；DOM有G但无编辑|**业务失败**，原request禁止重放；下一实验用新request|
|清理|34条原durable＋独立最终A关闭第35条均terminal/released/no pending；三个target detach；自有Chrome退出0，oracle143|用户Chrome/Arc不kill；只剩原desktop target|

原图仅本机证据，不随Git提交；不存在的早期01/02文件不作为交付。依次回读：

### A计数1

![A计数1](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/browser-window-lifecycle-r91/05-a-counter-one-original.png)

### B独立计数与文本

![B独立计数与文本](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/browser-window-lifecycle-r91/06-b-counter-and-text-original.png)

### C原生Incognito输入前

![C原生Incognito输入前](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/browser-window-lifecycle-r91/07-private-c-before-native-input.png)

### C输入后

![C输入后](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/browser-window-lifecycle-r91/08-private-c-after-native-input.png)

### C滚动后

![C滚动后](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/browser-window-lifecycle-r91/09-private-c-after-scroll.png)

### A真实后台：值未变化

![A真实后台：值未变化](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/browser-window-lifecycle-r91/10-a-background-input-original.png)

## 部署、耗时与输出

最终签名Agent11922／instance9f1a58be-6b2d-46ba-a93d-03c7de7f0b27，installed SHA256`a9be7414e0a821be5ef7b81b57f0b909bfc3c3a11c7f215d1c5c84e8377ae2d8`。MCP未替换、稳定本地证书／DR、配对、profile等保护hash未变，四项权限仍true；无需Codex重启。旧首部署94937仍作为历史失败证据，不冒充最终部署。

5次顺序真实只读snapshot：caller wall248–452ms，中位267ms；adapter pre-cache232–427ms，中位245ms；native capture wall107–177ms，中位115ms。文本9699–9701 UTF-8 bytes、原PNG235181 bytes分开计量。CPU、转换、编码没有独立测量；不是tokens数量、受控前后对照或语言性能优势。

Live保留**R73已部署/已验**的max280×300点（当时实际280×269），不降低1320×1104源捕获。两枚28点按钮：隐藏／恢复toggle（eye.slash/play.fill）与关闭xmark，tooltip与AX名俱全；不是同时三枚按钮。隐藏清图、恢复新帧、关闭停止采集保留旧独立Host/native证据，R91没有重新声称用户看见。设计参考[Apple窗口指南](https://developer.apple.com/design/human-interface-guidelines/windows)；没有公开Computer Use浮层精确像素规范，不能称1:1私有实现。

## 回归与同四角色事务

最新实际fmt／workspace test／全target全feature Clippy／架构／diff共9门禁均exit0，workspace **828 passed /0 failed /11 ignored /43 suites**；浏览器oracle9、receiver89为另跑例程，不叠加进workspace总数。源hash冻结与installed匹配。各次launch/timeout/socket WouldBlock/旧checkpoint重复部署拒绝、operator API/写图限制都留在证据，不把它们混成native输入失败，也不删除失败来提高成功率。

源copy同输入行为：hit及label BASELINE1→MODIFIED0→独立ROLLBACK1→REAPPLIED0；oracle check-only101→0→101→0；旧累计配置1→0→1→0。这些已实际完成并在`FINAL_REOPEN_R91.json`回读，属于源码/配置证明，**不回滚已安装Agent、不重放GUI**；累计patch已重建全部226 managed源，rollback恢复原hash/原缺席。

四固定角色：
- `/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/MODIFIED_FILE`
- `/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/DIFF_FILE.patch`
- `/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/VERIFICATION.txt`
- `/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/ROLLBACK.sh`

## 后续（全范围仍保留）

已创建R92候选、三个实际offline baseline0，尚未实现或验收新BG通道。下一最小改动是显式opt-in macOS experimental SLEventPostToPid down/up单对；无raise/激活/defocus、primer、Command modifier技巧、timestamp field58覆盖或自动fallback；新绑定／新request以独立DOM计数、AX和前台接收端验证。参考的no-raise会使原前台key window失焦，不能仅靠NSWorkspace前台PID不变就当无干扰。

随后仍按F01–F16继续cold hover、键盘实际消费与前台保持、dynamic hidden/minimized、跨provider窗口族、lost-release恢复与分层性能/MCP精简。保留R47／R49／R73／R84／R86–R90既有完成结果；R89已完成BG→FG原生DnD，不再把旧R88取消样本当当前待办。无需人工配合，不宣称整计划完成。

完整machine ledger：`/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/browser-window-lifecycle-r91/PARENT_VERIFIED_BROWSER_R91.json`。原请求与literalstdout/stderr、hash和角色最终回读保存在同一事务与补充记录。
