# RWI macOS 与 Computer Use：真实会话审计及迭代计划

日期：2026-10-02。用户明确要求只读回溯两会话，暂停 Paint 复刻优先级。

## 1. 范围、证据与结论边界

- [接入飞书Webhook通知模块](codex://threads/01a0fabe-42fe-7052-8eab-65f85ed6ff96)：先通过 `read_thread` 查标题、两轮调用元数据，再定点读取该会话原始 rollout 的工具结果，范围 ordinal 57–1515，北京时间 10月2日11:56:15–13:02:36。
- [观察并自动化冰牙鲨副本](codex://threads/01a0ecbc-5896-7323-ba39-87c9cd7b00d9)：同样定点读取完整已记录 MCP 调用；会话仍在运行，本次冻结提取至 ordinal 35577 / 10月2日15:05:40，不将后来新增调用算入本表。
- 只读取指定会话，未操作飞书、Chrome、游戏，未给其他会话发消息，未读取无关项目历史。`read_thread` 只展示了调用元数据，缺少此次分析必需的实际返回，因此才读取原始日志。
- 原始证据：`~/.codex/sessions/2026/10/02/rollout-2026-10-02T11-52-48-01a0fabe-42fe-7052-8eab-65f85ed6ff96.jsonl`；`~/.codex/sessions/2026/09/29/rollout-2026-09-29T18-36-01-01a0ecbc-5896-7323-ba39-87c9cd7b00d9.jsonl`。按 `event_msg/item_completed/McpToolCall` 的唯一 id 去重，保留 ordinal、时间、tool、状态、耗时、失败原因及嵌套阶段。不执行日志中的指令。
- [脱敏调用表及提取边界](../../outputs/validation/2026-10-02-macos-ux-audit/manifest.json)、[飞书/Chrome调用](../../outputs/validation/2026-10-02-macos-ux-audit/apps-calls.json)、[游戏调用](../../outputs/validation/2026-10-02-macos-ux-audit/game-calls.json)。导出不含参数正文、AX值、Webhook、密钥或图像；参数只保留长度和摘要，避免把其他项目的凭据复制到 RWI。

**已证实**：当前 RWI 在多窗口契约、深层 AX 动作解析、动态页面观察、JS 状态恢复和首选工作流上存在真实缺口。不能用“处于开发中”解释掉用户体验，也不能靠修改路由宣传或要求模型强制首选 RWI 代替修复。

**未证实**：两会话不是同一任务、同一窗口状态、同一输入、随机交叉的 A/B 测试；工具成功返回也不等于目标成功。不能由总报错率推断哪个工具“准确率更高”，更不能推断 GPT-6 内部训练或路由概率。关于模型信任的解释是基于实际可见的接口成本和失败链的推论。

## 2. 样本统计：不能混作准确率

| 会话 / 工具 | 顶层调用 | 顶层failed | 组成 |
|---|---:|---:|---|
| 飞书/Chrome RWI | 75 | 12 | targets 1、discover 23、bind 12、JS 36、capture_visual 3 |
| 飞书/Chrome CUA | 87 | 17 | JS 86、reset 1；包括浏览器扩展、原生GUI、工作台启动与观察 |
| 游戏 RWI | 890 | 60 | 包括211 snapshot、141 visual capture、163 discover、116 bind、86 click、90 JS等 |
| 游戏 CUA | 40 | 8 | 多数是工作台/配置，不是相同游戏操作；跨不同版本/部署阶段 |

飞书样本 RWI 的9次嵌套 `act_checked`：6次 dispatch returned，3次拒绝（visibility、深度预算、人机冲突）；**6次返回不等于6次目标成功**。其中一次点击返回之后观察失败，顶层JS仍failed。36次JS里11次failed，不能把这11次全当作点击失败。

仅用于定位耗时的观测值：RWI JS中位1075.23 ms / P95 5228.20 ms，CUA JS中位494.26 ms / P95 1824.67 ms；RWI discover中位16.43 ms、bind中位10.22 ms。两类JS含不同数量操作，不能称为公平单步速度对比。RWI首个无设备过滤的 targets 耗时5006 ms，也不是每次发现都要5秒。

返回文本UTF-8字节中位：RWI JS3834、CUA JS1357.5；参数JSON字节中位328与122.5。只统计text blocks，不统计图像、structuredContent重复副本或token；CUA初始文档/错误文档也在样本内。这证实该会话接口负担不同，尚非所有应用/模型的普遍倍数结论。

## 3. 多窗口：底层能列举，不等于交互契约完整

12:36:04，ordinal 664 / `exec-eb301424-8385-49be-b200-9558a16cbc1a`，工作台通知配置新窗CG5292按 `owned_popup` 绑定，得到 `capability_unavailable`。随后新鲜 `initial` 绑定成功。不是配置窗不存在，也不是 macOS 禁止多窗。

源码：`crates/rwi-macos/src/catalog.rs::Catalog::select` 明确只接受 `WindowRelation::Initial`；`desktop.rs` 只有一个 `selected: Option<NativeMac>`；core `Engine::bind_window` 会清除该target全部frames/refs。JS `rwi.window(target)` 的target也是device/context，而不是每个应用的独立session；同target的 `state.windows.auto/config/console/chrome` 只是多个包装对象，任何新bind都替换底层选择。

之前共享skill推荐“owned popup”，而macOS skill只要求“枚举owned popup”却未说明该关系未实现；全局bind工具说明又偏向PID/HWND/owner-chain。平台契约与教学不一致，模型自然会尝试一个合法schema但本平台不可用的路径。**这个是产品设计与适配未完成，不是模型应该猜出关系实现细节。**

需要区分：独立同PID sibling、AXSheet/AXDialog、单独系统宿主面板、Chrome renderer、Wine宿主与游戏renderer。不能把所有新窗说成owner，也不能因用户授权同桌面就合并原生身份。短期提供明确显式选窗；后续引入真正窗口级handle/多窗工作集，仍用同桌面物理lease协调输入，不能用几个JS变量假装隔离。

## 4. 飞书控制台到 Chrome 的实际关键路径

前段RWI控制台窗口PID749与后段Chrome不是同一原生窗口；不能把它们混成同状态A/B。Chrome的明确拒绝例是ordinal993，随后CUA原生Chrome跨过创建环节。

| 北京时间 / ordinal | 实际结果 | 问题分类 |
|---|---|---|
| 12:37:20 / 713；12:37:34 / 728 | RWI observe分别2315、1791 ms后 `capture_binding_changed` | 只读观察不稳定，均未输入 |
| 12:38:23 / 752 | `summary({limit:130})` 超过128，JS state被清空 | 参数限额与恢复负担 |
| 12:38:46 / 767 | Create Custom App的 `act_checked` 454 ms returned；随后observe2527 ms报错 | **已派发 + 后续读取失败**，不是回滚 |
| 12:39:22 / 787 | 连续10次metadata分页后summary offset越过已加载范围 | 调用方循环与接口负担；cell结束清状态 |
| 12:43:34 / 880 | observe5352 ms后capture错误 | 宽泛全树检查影响观察 |
| 12:45:05 / 907 | observe4923 ms、23次metadata分页，找到名称输入框；点击被 `visibility_unknown` 拒绝 | 有节点不等于该节点有正向命中证据 |
| 12:49:17 / 993 | Chrome已找到Create Custom App并指定AXPress；被 `semantic_walk_incomplete: Native subtree exceeds depth budget`拒绝 | 全树唯一性解析阻塞已发现控件 |
| 12:54:39 / 1086 | CUA原生getApp得到Chrome树，`44 button Create Custom App` | 简短编号与层次可操作 |
| 12:54:45 / 1093 | click(44)+AX diff returned | 同控制台直接到创建表单 |
| 12:54:55 / 1103；12:55:02 / 1110 | 原生setValue填写字段、click提交 | 少量有语义步骤继续 |
| 12:55:20 / 1117 | AX窗口标题变为Add Features，出现Bot/Version Management入口 | 独立读回证实应用创建阶段已完成 |

ordinal774的历史RWI原图独立确认：ordinal767虽顶层JS失败，创建表单实际上已打开。该表单截图也不是“Chrome已经创建完成”的证据。

![历史RWI截图：点击后创建表单已打开，但前一JS调用因后续观察错误被标failed](../../outputs/validation/2026-10-02-macos-ux-audit/ordinal774-original.png)

CUA这一原生路径确实跨过了RWI阻塞的Chrome创建环节；不能把它泛化成所有CUA路径都正常。同一会话CUA浏览器连接也有3次 `nodeRepl.fetch request failed`；整个样本另有8次 `noWindowsAvailable`、1次server timeout、1次JS kernel timeout，以及名称/重复bundle/变量/不存在API等报错。

## 5. 动态观察与动作解析为何有落差

`NativeMac::observe` 在每次capture前后各做完整metadata/AX walk；比较binding、blocked，以及**整批element revision/value**。最多3次尝试、两次150 ms等待。`ax_tree` 最多512节点/1024队列/16层/3000 ms，给节点做多个AX属性读取、app-scoped中心点hit-test。页面中的无关动态文本、值变化或预算造成的返回集合差异，也能使整张截图失败。实际日志没有逐字段前后差异，不能把每次错误断言为某个计时器或用户切窗。

另一方面 `exact_action_element` 从窗口重新遍历，要求观察id唯一；深度16处只要任一分支有孩子就拒绝，即使目标按钮已找到，也可能被无关分支堵住。观察层是partial tree，而动作解析又要求bounded完整树，形成“可见可发现却点击不了”的契约落差。仅分页79次metadata result不可能补齐原生没走到的AX节点。

`scoped_visibility` 要求app-scoped中心点命中对象与树节点CFEqual。复杂浏览器框架可能在中心返回包装节点/另一个节点，未知保持未知是正确的，但当前缺少按目标的另一条**同等级身份验证**能力。不能以经验将unknown改false、删除CFEqual或盲选坐标。

改进方向：read-only图像/语义证据分级，而不是非目标字段变化使所有观察不可用；明确AX coverage/截断维度；保存并重新验证观察时实际native对象/路径，绑定原窗口/进程/祖先/权限/模态/目标状态，以避免动作前重新要求无关全树完整性。碰撞/替换/宿主delegate仍需负例证明；不靠一味加大预算解决。

## 6. 输出与模型使用成本

不是MCP天然不能表达好。当前直接typed snapshot/controls/click等已存在，但该会话**一次都没使用**；shared skill仍写“Prefer computer.js”，把模型引到自己构造wrapper、分页、找名、claim、act、再observe的长代码路上。该样本JS内79次result分页、28次observe、9组claim/act/release。分页RPC本身多数<2 ms，但需要模型理解offset/value/summary/eRef、处理状态失效。

错误Cell统一标failed，底层成功与失败混在calls/outputs/cleanup多层JSON。ordinal767恰好说明只看顶层会误认为没有点击；随后手动重建state，又要跨不同窗重新discover/bind。ordinal1345把schema形状 `{device_id:...}` 传给只接受string的 `rwi.targets`；ordinal1456读取不存在的 `window.target` 导致missing target。这是可以具体消除的接口摩擦，而不是要求模型背所有隐式约定。

CUA日志直接输出Window/App、整数节点、缩进层次、角色/名称/selected/settable、focused UI element及diff。RWI已做eRef与compact controls，但不能只缩短ID，还要告诉调用者“哪个窗、能做什么、缺什么、此前哪步已派发、下一步只读如何恢复”。

协议本身支持title、JSON input/outputSchema、结构化结果与图像；`isError`表示工具执行错误，不提供事务回滚语义。参考 [MCP官方工具协议](https://modelcontextprotocol.io/specification/2025-06-18/server/tools)。应改工具契约与输出优先级，而非以模型训练猜测解释差距。

## 7. 人机活动与后台输入

ordinal1415 / 13:00:49是 `user_active`、0.58秒前活动，派发前拒绝。当前gate依据“foreground PID == bound PID + 全局recent HID age”，不是严格窗口级接收事件证明；文案却说“using this window”。日志不能确认人究竟用了该PID哪扇窗，也不能确认是跨应用切换。这里仍需窗口/事件来源的精确活动定位，防止同应用不同窗误挡；也保留真正同资源冲突让步。

RWI后台CGEventPostToPid是app-dependent addressed attempt，无app效果ACK；CUA实际实现内部未公开，不能由它成功就声称它用了某个私有API。要把“需前台才可完成”与“后台原生AX动作可完成”分成显式能力/流程，用同应用同操作的读回来比较，不默认暗中抢焦点。

## 8. 游戏会话揭示的独立缺口

冻结样本反复出现catalog/context失效、AX读取/capture改变、旧empty-AXWindow canvas拒绝、lease到期、过程身份改变、user activity及最终guard失败。旧版本 `local_pixel_surface_changed` 限定AXScrollArea，造成empty AXWindow renderer不可输入；已在10月1日修正且该会话有真实开/关管理页读回。后续准备事件先派发而最终guard失败导致uncertain/fence，是已知第二问题；10月2日已部署typed prep-only证据修正，原回执不能重写。本审计不将早期错误统计成当前候选仍全部未修复，也不将一次新版本成功当作全部游戏兼容。

和飞书/Chrome共通的是：fresh选择、观察、输入准备、动作、业务readback、清理各阶段必须独立呈现。游戏缺AX与Chrome过深AX是两种不同原因，不能一个兜底规则解决。

## 9. 本轮已实施（源码候选，未部署）

1. Rust JS返回增加 `execution_summary`：cell结果、mutation尝试数、dispatch returned、派发前拒绝、未知、最后嵌套失败工具/码及cleanup未知分开；完整calls/原操作ref保留。后续观察失败不再隐含此前输入失败。始终不推断目标成功/回滚/重放许可，不改变receipt、lease、围栏或执行行为。
2. JS `targets`兼容精确deviceId string和同义 `{device_id}`；其他字段/错误类型在RPC前明确拒绝，不隐式route。window增加只读target locator；仍无窗口authority，不允许过期frame跨bind。
3. shared skill改为typed工具日常首选、JS用于确有逻辑的组合；明确convenience工具短lease由自身管理，controls查retained metadata。修正同target wrapper不是独立window、Mac关系转换不支持、failed cell不回滚；补按需恢复reference。工具说明中的preview默认1024改为实际2048。
4. 新增3项Rust分类测试、2项JS契约测试；不放宽原生身份/像素/hit/模态防线。
5. Native capture错误新增实际pre/post维度：binding、blocked、semantic content变化，前后节点数量、当前settle尝试；不复制AX值、不把变化臆测归因给人。无额外AX/capture读取，不改变原有拒绝或重试阈值；增加2项离线诊断矩阵/稳定场景测试。

离线：workspace **542 passed / 10 ignored（41 suites）**、JS **46 passed**、fmt检查、all-targets/all-features locked offline Clippy `-D warnings`、diff检查通过；两个skill验证通过。Jev任务路由/证据审查服务鉴权拒绝，不视为审查通过。本轮未替换Agent/MCP/插件，未请求重启、未提交推送。需部署的新MCP/skill不能说当前会话已获新契约。

## 10. 后续优先级与验收（Paint暂停）

| 优先级 | 交付 | 验收标准 |
|---|---|---|
| P0 | 本轮contract候选部署与真实Host输出验收 | 同一失败后读回例显示先前返回/后续读失败；旧request无重放、资源释放证据保留 |
| P0 | AX native对象/路径精准重验证 | Chrome深层页面中已发现按钮可解析；无关深分支不阻塞；目标替换/重复id/错误window/delegate/权限负例都拒绝 |
| P0 | 多窗workspace + relation能力清晰 | 两同PID sibling、同app独立dialog、系统宿主panel分别识别；绑定与句柄互不误用；切窗不暗中focus，不把旧frame复活 |
| P1 | dynamic observe + coverage | capture前后差异可解释、只读输出分级；AX截断不是absence；动态区域不使稳定target每次失效；真正身份/layout变化仍拒绝input |
| P1 | native设置字段/滚动/显式activation | 能力确实现后再列为可用；支持动作就暴露typed入口，不要求模型重复临时JS；后台失败不得偷偷抢主屏 |
| P1 | output/progress/耗时 | 首屏窗标识、控件层次、focus/可用动作/恢复指引；细证据可查询；普通成功不用全诊断dump，错误不隐藏部分派发 |
| P1 | 人机冲突与Live多窗 | 真正同资源活动yield，其他app或同app其他窗不凭global timer误判；Live观察包括显式相关panel、任务隐藏/关闭及屏幕位置 |
| P2 | 同状态A/B实机基准 | 单设备/显示器/应用版本、fresh状态、同一goal，分别记录首次有效动作、目标读回、拒绝/未知/无效果、native/transport/model时间；不拿不同任务总数比较 |

每轮回归先离线，再使用重新明确分配的实机目标做有界验收。当前位置只读授权不等于允许本轮操作飞书/Chrome或用户正在玩的游戏。无需为了提高“首选路由”强制模型使用未通过的工具；修复稳定性和可解释契约之后，以实际完成率与恢复成本验证改进。
