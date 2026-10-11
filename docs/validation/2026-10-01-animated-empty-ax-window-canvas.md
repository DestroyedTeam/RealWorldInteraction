# macOS 自绘游戏窗口的动画点击适配（已部署，游戏点击效果待验）

来源：「观察并自动化冰牙鲨副本」会话 `01a0ecbc-5896-7323-ba39-87c9cd7b00d9`，用户明确要求反馈重复拒绝并修复。目标PID61792、CGWindow1671、bounds `[0,0,1800,1169]`。来源[拒绝报告](</Users/vincentadamnemessis/Documents/ChatGPT/Strange Beasts General Store/logs/qa-boss-strategies-20261001-2138/rwi-click-rejections.json>)记录三个fresh view下的“奇兽”和“骑乘”点击，均`local_pixel_surface_changed/rejected_before_dispatch/native_dispatch_entered=false`；该会话没有改走其他输入路径。

## 原因与独立事实

- Core的局部像素证据已通过，失败在macOS `input_anchor_facts_in_sequence` 后续只接受`AXScrollArea`的角色判断；没有这些控件的自绘窗口会被确定性拒绝，重复fresh capture不能消除角色限制。valid view表示原生窗口/像素引用可解析，不代表所有输入前置条件或应用效果已经满足。
- 对分配的PID61792运行现有Rust只读AX/CG诊断，明确清除`RCU_DIAG_CAPTURE`，未截图、输入、变更绑定或重启服务。实际返回`AXWindow/AXUnknown`，`AXChildren=Some(0)`，`AXModal=false`，中心application-scoped hit的PID61792、window1671且`CFEqual(hit,window)=true`。`AXWindow`父属性未返回值，但根对象自身的窗口ID存在；不能把父属性缺失当作根窗口身份缺失。[只读记录](2026-10-01-animated-empty-ax-window-readonly.log)。这只是原生结构证明，尚不是游戏点击效果验收。

## 修正边界

保留原`AXScrollArea`路径，新增严格的window-level canvas分支，仅在已有局部像素证据被使用时检查：

1. `AXUIElementCopyElementAtPosition`的application-scoped真实命中，不能来自几何树回退。
2. hit角色为`AXWindow`，与选中的AX窗口`CFEqual`。
3. hit和窗口的PID均与绑定PID相同；两者原生窗口号均等于绑定CGWindowID。
4. `AXChildren`属性必须成功读取并为明确的空数组。unsupported/no-value/timeout/malformed/超过预算均为unknown，不推断为空。

局部像素匹配阈值、原生进程/会话身份、窗口/命中对象与几何、显示器映射、modal、键盘精确焦点、权限、单输入租约和不可重放保留。按钮自身像素改变仍可在Core停止；纯色区域仍要求整窗一致。AX窗口根不可用的`visual_only`分支继续只读。这不保证游戏接受CGEvent，也没有增加没有原生可编辑控件的键盘/文字输入能力。

错误继续保留原code以兼容，但现在给出实际role和各项window canvas proof，不再让零控件游戏反复尝试不存在的advertised element action。capabilities新增`local_pixel_grounding`，明确这些条件、纯色区规则和visual-only不可输入。

Apple定义[`AXChildren`](https://developer.apple.com/documentation/applicationservices/kaxchildrenattribute)为直接子AX对象数组；[`AXUIElementCopyElementAtPosition`](https://developer.apple.com/documentation/applicationservices/1462077-axuielementcopyelementatposition?language=objc)执行命中测试，传application对象时范围限定在该应用，仍考虑窗口层级。以上准入组合是RWI的实现策略；实际窗口身份和child数据来自本机诊断。

## 验证与部署状态

- 新增角色/证明矩阵负例：其他role、几何回退、不同CF窗口对象/PID/窗口号、未知/非空children均不能进入新分支。
- 新增离线跨层fixture：零controls、稳定按钮和其他区域动画可派发一次；重复request只取保留回执，不再派发。按钮移动或native children unknown则零派发。这些是synthetic admission测试，没有真实GUI输入。
- macOS crate **96 passed/6 ignored**；全仓离线 **533 passed/10 ignored**，fmt、全目标/全特性Clippy `-D warnings`、release Agent构建、diff check通过。
- 同leaf本地签名候选 `work/animated-window-canvas-20261001/RWI Agent.candidate.app` 通过严格验证；用户回复“现在部署，随后刷新游戏绑定”，游戏会话确认输入worker空闲、在途RWI调用和租约均已结束并暂停新调用，之后备份旧安装、替换应用、重新启动同一LaunchAgent。安装后可执行文件SHA256实际为 `b59d96a375adc0d64a0b193da80b94438ae286b49955b38e0e7c0cd733a5ed0a`，与候选一致。旧安装SHA256 `e33c39ddfd461f67b4cf2c3f1e0262d6b7f60d10cbb5c1f0c45d3589e77d231e` 及备份保留在 `work/animated-window-canvas-20261001/`。本阶段未提交/推送。

## 部署后实际只读验收（2026-10-01）

1. 安装路径仍为 `/Users/vincentadamnemessis/Library/Application Support/RealWorldInteraction/agent-mac-v1/RWI Agent.app`。`codesign --verify --deep --strict` 成功，designated requirement仍是identifier `com.destroyedteam.real-world-interaction.agent`＋leaf `24b346a95ec8f450f7d8433a2e1876568dbcb2f8`。未重置TCC、修改配对CA或MCP/Plugin设置。
2. 当前已加载MCP直接读到新Agent **PID64545**、instance **`d738b7f6-8623-4359-937f-ca17da6da636`**、service `healthy`，`input_domain_fenced=false`；Accessibility/Input Monitoring/Post Event/Screen Capture均true，screenshot API可用。LaunchAgent PID独立相符。旧instance `572452a8-6721-4f47-8fd9-1525a737252d` 的target/frame/view不再使用；此次无需重启Codex。
3. 新 `computer.targets` 实际返回 `local_pixel_grounding.native_surfaces` 两个准入分支，并明确empty-AXWindow证明要求；这是部署功能暴露验证，不是输入效果。
4. 新target **`mac-host:d738b7f6-8623-4359-937f-ca17da6da636:39addf0f-8c95-4b9c-bd85-b162e37ad9ba`** 下重新精确发现并绑定PID61792/CGWindow1671，fresh discovery的process start `61792:1790826329:801087`、native epoch `f9e9cfa03d0fa3f8da60574f35d9eb14d942c3cd5abfd826b4f3732620960bc6`、bounds `[0,0,1800,1169]`，仍在内建主屏。发现和绑定均`activated=false/input_sent=false`，操作资源已释放；没有移动游戏窗口。
5. 实际只读 `computer.snapshot(max_edge=1512)` 交付 **1512×981** preview，原图 **3600×2338**，`visual_only=false/blocked=false/mutation_ready=true`、0controls、有新frame/view，总耗时2454ms（observe1976ms、image477ms）。画面确认为CreatureCurios游戏而非启动器。frame `ea82f3d1-6555-4a1b-b44b-e11affe3ecab`、view `f654e87e-796b-4c37-afce-d163143c9bbb` 仅记录本次证据，不能交给另一会话重放/长期复用。

实际[部署后游戏截图](../../outputs/validation/2026-10-01-animated-empty-ax-window/01-post-deploy-game.png)及[只读机器证据](../../outputs/validation/2026-10-01-animated-empty-ax-window/02-deployment-readonly.json)保存在outputs，不加入Git原始截图。**本任务未对游戏发送点击/键盘，也未验证“奇兽/骑乘”页面切换。** 新snapshot可操作状态并非业务成功证明；游戏会话恢复后应自行新targets/discover/bind/observe，并以单次新请求和独立页面变化验收，不使用这里的短期frame/view。

后续：部署与本任务只读刷新已完成；游戏会话在其用户分配步骤中重新绑定、限定一次新点击，以页面变化独立验收。不能重放上述三个旧请求，也不能把离线派发回执当作游戏页面效果。Paint V14的215对象可编辑源未改，复刻与其他实施项继续保留。

## 后续反馈：来源游戏会话已验证关键点击

来源会话随后反馈“奇兽”实际打开管理页，“休息”后原生复读读到“骑乘”，关闭和重新打开也成功；这是其真实GUI验收，不是本任务发送或重放这些操作。随后另一次关闭请求触发最终native `stale_reference` 与准备事件相关的误围栏，本任务独立durable query确认并修正候选；在来源会话转为不发RWI游戏输入的只读时机于北京时间10月2日00:03完成同签名部署，新PID92469/instance 08ac04cf…，但新最终失败路径未实机复验。详见[最终检查诊断/准备事件围栏部署](2026-10-01-game-final-guard-preparation-fence.md)。上述首轮部署时的原截图/时间/“本任务无输入”记录保持原样，不将后续效果混记为当时已验。
