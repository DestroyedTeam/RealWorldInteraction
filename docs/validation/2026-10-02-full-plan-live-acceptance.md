# 全计划首轮实机验收：执行、观察、耗时与工具契约

日期：2026-10-02。接续[完整审计与 P0–P2 计划](2026-10-02-macos-computer-use-comparison.md)，不是把计划缩成多窗口，也不是总体验收完成。用户已授权部署及 Chrome/飞书/游戏等验收；本轮实际输入仅针对自建本地 Chrome fixture，游戏、飞书业务配置及 Paint 源未操作。共享 RWI Agent 的重启会使其他调用方的旧绑定失效，应用本身未重启。

## 1. 部署与门禁

- 最终 Agent PID **58619**，instance `cc53decd-9185-4671-ba43-22fdbe6f6ed2`；安装 Agent SHA256 `5a2857960fcb52460343f007811ab32fa607d081a0427311ef1a1045eef94f88`。原 APP 路径、identifier、同一本地 leaf 签名要求保持，严格验签通过，四项权限仍 true，围栏 false；没有修改 TCC、CA、配对/profile 或导出私钥。
- 安装 MCP SHA256 `f5291393148a98764515cb75a9177acf8daaa63f3290f922d2c01118a3d0e511`；个人 Plugin 源 `~/plugins/real-world-interaction` 已重新打包。此任务仍加载旧 Adapter/Skill：**34 个工具**；新生产二进制的实际 stdio catalog 测试为 **37 个**，新增 attach/detach/scroll。安装不等于 Host 重载，新工具首屏不能算本任务实机通过。
- 最终 workspace **572 passed / 10 ignored（41 suites）**；JS **50 passed**；fmt check、locked/offline all-targets/all-features Clippy `-D warnings`、diff check、release 构建、两个 Skill 结构验证通过。新增 scroll 曾因 stdio catalog 的精确预期未更新而失败；已补新工具及无效参数/title 检查，重跑全仓通过。Clippy 的 `useless_format` 已修复后重跑，未抑制 lint。
- Jev guard 仍鉴权拒绝，没有有效审查 verdict。备份 APP/MCP/Plugin 保留；未提交、推送、发布或触发 Actions。机器态详见[最终 manifest](../../outputs/validation/2026-10-02-full-plan-live/final-manifest.json)。早期 `deployment.json` 是历史部署，不代表当前 PID。

## 2. 实际失败链与修复，不把 returned 当成功

### 2.1 找到了按钮，但显式 AXPress 被误挡

初始 Chrome A（PID58489 / CG5890）已发现 Increment A，Count 0。两次显式 AXPress 在派发前拒绝；匿名诊断确认 window/scope/focus/layout/epoch/content、原对象、point、name/role/actions/enabled/value/state revision 均未改变，仅 app-scoped pointer hit 导出的 visible/occluded 波动。

![01 初始 Count 0：截图成功，但 AXPress 两次零派发拒绝](../../outputs/validation/2026-10-02-full-plan-live/01-chrome-fixture-preview.png)

原契约已允许显式 advertised native action 不依赖鼠标中心命中，但 Element revision 又把那两个 pointer hint 混入语义动作哈希。修复仅使显式原生动作 revision 不包含这两个 hint；其他字段、真实 AXHidden、CFEqual 根/对象、provider/process、祖先、权限、模态、几何与 action 检查保留，普通鼠标/文本/键盘路径不放宽。

部署后新 request `rwi-full-plan-increment-fixed-20261002-3` 返回，**独立截图读回 Count 0→1**。原拒绝回执及 request 保留，没有重放。

![02 修复后独立读回 Count 1](../../outputs/validation/2026-10-02-full-plan-live/02-fixed-count1-preview.png)

### 2.2 深层分支、动态文本与后台窗口

- A 加载 24 层显式 ARIA group：CUA 可读到 Deep group 23 / Deep button；RWI 初始树到 depth16，仍没有 Deep button。**已发现的浅层 Increment 不再被无关深分支阻塞**，再次 Count 0→1；并未宣称深层覆盖完整。
- 75ms 无关 timer 启动后，三次 snapshot 成功（590/537/663ms），均明确窗口稳定、像素/语义非原子；同时按钮动作成功，Count 1→2。原方案把整树值差异当作 capture 失效；现在只读图像/后读控件分级，真正 window/context/modal 变化仍拒绝，每个输入仍按原目标重验证。

![03 动态区域运行期间独立读回 Count 2](../../outputs/validation/2026-10-02-full-plan-live/03-dynamic-count2-preview.png)

- 原生输入写入 `RWI 测试字段 2026-10-02`，独立 snapshot 和 CUA 语义树均读回完整值。该次字段位于可操作状态，不据此声称被覆盖字段也能后台写入。
- CUA 只读证实 Chrome 当前窗口为 B（CG5918）；A 的 Increment 被同应用 B 遮挡，pointer hints 为 visible=false/occluded=true。RWI 对 A **显式 AXPress** 成功，Count 2→3，CUA 随后仍选择 B。普通 pointer click 对同一 A 控件保持 `visibility_unknown` 零派发拒绝，没有 fallback。
- 最终单树版本重新绑定 A 后，动态运行时又独立读回 Count 4→5；timer 已停止，最终重新绑定后仍 Count5、中文字段完整。这是 covered-window 的语义动作案例，不是所有后台鼠标/键盘兼容证明。

![07 最终版本 Count 5；普通 overview 是1600px预览](../../outputs/validation/2026-10-02-full-plan-live/07-final-count5-preview.png)

[1500×1200 原始像素 tile](../../outputs/validation/2026-10-02-full-plan-live/08-final-native-tile.png) 的 mapping 为 `native_resolution_tile`、1:1；不是把 overview 放大冒充原图。对应窗口完整源为3024×1900。

## 3. 性能：先定位阶段，再删除冗余工作

实测 snapshot 的主要成本是 capture 前/后各一次 AX 控件 walk，其次是原生 capture、PNG/preview；本地 discover 和 lease RPC 并不是每步数秒的主因。动态观察不再需要用前一整树作准入，因此优化为：**轻量原窗口/session/process/权限/geometry/title/layout/modal guard → 原生 capture → 一次完整有界控件 walk → 同样窗口/context/modal比对**。输入 revalidation 不走轻量 guard，仍读取原对象及完整目标准入；未增加后台激活。

| 同一本地 A fixture、同尺寸/max_edge1600 | 3次 Adapter snapshot elapsed（ms） | 中位数 |
|---|---|---:|
| 两次控件 walk，Count3 | 656 / 568 / 566 | 568 |
| 单次控件 walk，Count3，优化后首次采样 | 440 / 447 / 427 | 440 |
| 最终活动范围收紧版本，Count4 | 476 / 445 / 464 | 464 |

第一组与第二组窗口状态/字段/Count 相同，本地非随机、每组仅3次；观察到中位降低约22.5%，**不作整体性能优势/P95结论**。最终版本读回中位464ms为另一次烟测，不与 Count3 冒充同像素状态。前置 metadata 原143–183ms，单树为4–20ms；post-tree 仍143–170ms，capture169–200ms，preview77–88ms。成功按钮耗时97–219ms有波动，不能用26–47ms的拒绝当作快速成功。

`semantic_content_changed:null` 明确表示未收集前一控件树比较，不是“内容没变”；`control_tree_walks:1`只计控件 walk。coverage 返回 visited/described、深度/队列/节点/时间预算、属性读取失败等实际维度。当前 A 的 depth_budget、children_read_unavailable、element_read_unavailable 仍明确暴露，不将截断当 absence。

CUA同 fixture 初次成功 action153ms、action+read600ms，178ms和406ms是**不含图像**的语义读取；不能直接对比含捕获/preview的 RWI snapshot。CUA还出现初始 inventory timeout 和隐私窗 setup 的 stale-state guard；两者均保留，未隐去。完整分阶段样本见[优化前](../../outputs/validation/2026-10-02-full-plan-live/coverage-scroll-performance-before.json)、[优化后](../../outputs/validation/2026-10-02-full-plan-live/performance-after-one-tree.json)、[CUA对照及边界](../../outputs/validation/2026-10-02-full-plan-live/cua-comparison.json)。未测模型思考耗时，也未随机交叉A/B。

## 4. 工具、滚动与多窗口，分别验收

- 新 `computer.scroll` 接受一次 exact element 或 delivered view + delta_y，内部一次 resolve（需要时）、claim、act、release；拒绝不重试，原操作保留。Mac是有符号pixel±2000；Windows是120倍数wheel units±1200，不暗做单位换算。离线验证 native intent、清理、错误 view、零值/越界及实际MCP工具catalog。**新 typed scroll 尚未在此旧 Host 工具表实际调用。**
- 已加载 RWI 的底层 claim/act_checked/release 对 B 发送一次 -600 pixel scroll，返回后独立读回 **Scroll position 0→600**；不是拿新工具候选冒充 Host 验收。

![04 滚动前 Scroll position 0，1440×1252 native tile](../../outputs/validation/2026-10-02-full-plan-live/04-scroll-before-native.png)

![05 滚动后 viewport 改变；原控件 metadata 独立读回600](../../outputs/validation/2026-10-02-full-plan-live/05-scroll-after-native.png)

- CUA用于自建隐私窗 setup，不作 RWI launch/activation 证明。RWI独立发现 CG5971/PID58489，原生 Incognito 按钮及 bitmap均核对，窗口 `[1846,46,1512,950]` 在同一BetterDisplay虚拟屏；RWI绑定、点击、Count0→1读回成功。
- Root换绑后，对旧 A frame 的专门负例得到 `unknown_frame`、零派发、无围栏；没有把同PID/相似按钮当作同窗口。RWI原生 Close action返回后第一次discover仍看到关闭动画中的缩小窗口；没有重复 Close，后续fresh discover为0匹配，CUA回到普通B/Count0。恢复A后Count5及字段保留。

![09 实际隐私窗口，RWI读回Count1，非浏览器DOM模拟](../../outputs/validation/2026-10-02-full-plan-live/09-private-count1-preview.png)

普通A/B及隐私窗通过了“精确发现/旧式root换绑/原生动作/状态隔离”路径，**不是 attach/detach 独立工作集、系统宿主panel或Live多源已通过**。业务Chrome CG5318未移动/修改，所有fixture在既有虚拟屏UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`；该显示器不是独立键鼠session。

## 5. 人机隔离和 Live 的实际边界

旧活动注释把 combined-session timer称为“真实人类硬件”，不准确。[Apple官方](https://developer.apple.com/documentation/coregraphics/cgeventsourcestateid)区分 combined-session所有来源和HID硬件来源表；状态表不提供接收窗口，也不是不可伪造的人类证明。

本轮显式 native semantic action可在已知foreground/focused窗口为另一精确 sibling 时不继承其活动gate。**synthetic输入可能准备同应用的共享receiver，仍保留同PID保守yield**；未知窗口身份仍yield。1.2s阈值未放宽；拒绝输出combined/HID年龄、窗口身份已知与否、human_attested=false、source_class=unknown。没有把非RWI自动化归为可信来源，也未通过真实用户事件来源测试。GameIO误判反馈仍在计划内，不算已修完。

Live本轮start指定虚拟屏，但 `open_in_codex`返回 **queued**：任务面板未可见。实际status显示0 captured、presenter hidden/suspended；随后stop为stopped、image absent、worker_done。验证了“未可见不采集/停止清理”，**没有展示overlay，也没有用户切换任务/物理屏/Hide/Resume的实机验收**。没有伪造visibility事件或用其他浏览器替代任务面板。

## 7. 深层 AX 覆盖后续验证（同日追加）

此前同一 A 页面明确包含24层ARIA group。CUA 可见 Deep button A，RWI 初始16层walk只到group7。将 **控件树** 最大深度提高到40后，实际能力读回为40层；fresh snapshot耗时370ms、visited92/described90，maximum_depth_seen=33、24个Deep group均返回，`computer.controls`可按“Deep button A”直接找到；节点512、queue1024、3秒walk上限仍有效，coverage仍列明2次child和2次descriptor read失败，因此没有宣称整棵AX树完整。点位搜索仍保持自己的深度界限。

![深层按钮点击后的原像素tile；Count仍为5，说明receipt没有替代效果读回](../../outputs/validation/2026-10-02-full-plan-live/10-after-deep-axpress-readback.png)

对按钮发出一次显式AXPress，receipt显示`dispatch=returned`，独立AX读取和1:1像素tile都仍显示Count5，按钮仍在列表里。该测试的业务效果**未通过**，没有用相同按钮再发第二次操作。它把问题从“模型看不到控件”缩窄为“深层对象可发现，但被遮挡时该动作尚未证明会落到Web内容”；控件发现改善并未解决RWI/CUA整体执行质量。

本轮深度40部署 Agent PID83412 / instance `8a502d62-7b73-4292-91ed-db704c50dd38`，SHA256 `d21536f2580101f7dc79301caa5b4a40e507166e9f1ee477b9f6f87f029609c2`，同leaf严格验签、四权限保持、围栏false。Agent源码+MCPHost stdio gates：workspace572 passed/10 ignored、JS50、fmt/clippy/diff/release均通过。该任务仍加载旧Host工具表；这里验的是旧工具下的snapshot/controls/click，不是新scroll/attach工具的Host验收。

## 8. 剩余完整计划（继续，不以单项停止）

| 主线 | 下一交付及验收 |
|---|---|
| 真实日志/恢复 | 新Host验证typed-first、JS execution_summary 的先派发后读取失败例；原operation不重放、细证据可查 |
| AX/coverage | 初始深层树可定向展开/查询；target替换、同属性对象、错误root/provider等负例；不是单纯加预算 |
| 窗口workspace | fresh Host attach/detach独立A/B/隐私句柄；系统宿主色彩/字体/Open/Save、owner证据与旧ref负例 |
| 字段/滚动/焦点 | typed scroll Host验收；明确native属性设置、键盘焦点/activation能力与前后台效果，不偷偷抢焦点 |
| 性能/真实A/B | 更多应用与多状态重复样本，调用首次有效动作、goal读回、无效果/拒绝/未知及model/transport/native分别计时 |
| 输出/模型成本 | 新Host窗口标识/title/coverage首屏，层次/focus/settable与短引用/diff，错误恢复指引和部分派发始终保留 |
| 人机操作隔离 | receiving-window及输入来源证据，GameIO/已知本地自动化与真人活动区分；synthetic同app隔离仍未完整 |
| Live/显示器 | 两窗口/相关panel、任务真实隐藏恢复、Hide/Close、物理/虚拟屏位置及材质对照；当前queued不是验收成功 |
| 应用覆盖 | 飞书、Arc及游戏后续有界任务；本轮未对它们的业务操作/游戏点击宣称通过 |

以上同步进入[implementation status](../product/implementation-status.md)。新 Host工具/Skill 重载尚待用户选择重启或接力；本轮不因此把其余原生修复停下，也不改成“只剩多窗口”。

## 7. 深层 AX 覆盖后续验证（同日追加）

前述 Count3 fixture 中的 24 层ARIA组树、CUA可见 Deep button A、RWI只读到group7，为明确缺口。本增量将 Mac 控件树深度预算从16改为40；512节点、1024待处理节点、3秒walk上限、目标对象身份/输入/租约检查及点位搜索预算不变。能力结果现在报告40层。不是将4096改成无限，也没有把完整性强行标成true。

部署后：PID **83412**，instance `8a502d62-7b73-4292-91ed-db704c50dd38`，Agent SHA256 `d21536f2580101f7dc79301caa5b4a40e507166e9f1ee477b9f6f87f029609c2`。

## 9. 模型可执行的 AX Scroll-to-Visible 入口与当前 Host 差异

本增量同时覆盖“模型执行便利”和“真实窗口覆盖”，不拿一个新工具代替其余性能/隔离/Live计划：

- 实际 Host 当前暴露的 RWI MCP 工具表有34项，包含旧的`computer.click`/`computer.snapshot`等，但没有`computer.scroll`与本回合的新`computer.reveal`。虽然 Host 已重启过，MCP tools/list仍与源码候选不一致。RWI `computer.targets` 单端点只读状态确认 PID83412、instance `8a502d62-7b73-4292-91ed-db704c50dd38`健康，AX深度40；本次不改Host安装，不触发GitHub Actions。
- 使用该目标新鲜discover，只读枚举到本地fixture PID58489的A/B两个窗口：A `cgwindow:5890` `[1824,24,1512,950]`，B `cgwindow:5918` `[1800,0,720,626]`，显示器均为BetterDisplay UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`。明确bind A不激活、不发送输入。snapshot elapsed 398ms；source 3024×1900，preview 2048×1286；AX控件返回64项（本次首页limit50、剩余14未展示），`native_complete=unknown`，状态仍为Count5、原字段值仍在。多个A上的控件报告`visible=false/occluded=true`，包括`AXScrollToVisible`所见的深层控件；本回合没有再点深层按钮，也没有把遮挡原因归给B、OS或AX提供者。当前仍是带诊断数据的可观察状态，不作成功结论。
- 新增MCP typed工具`computer.reveal`：接收一个snapshot的精确frame与element，先只读确认该target capabilities明确含`AXScrollToVisible`，再claim、发送一个有durable request id的精确AX动作并释放lease。能力不存在时保证在claim和input之前返回`capability_unavailable`；不能退化成鼠标点击。返回标识`operation=reveal_to_visible`及`native_action=AXScrollToVisible`，`goal_verified`仍false，用户调用方需要新snapshot验证控件是否实际滚入可见区域。Agent候选相应声明该动作；尚未部署，故尚未对Chrome或其他App运行这一动作。完整stdio catalog预计38项。
- 验证：新增两项隔离测试覆盖成功路由（唯一act_checked、claim/release）及缺能力无claim/no-dispatch；RWI MCP stdio目录测试扩充新名称。首轮因目录精确白名单没有同步而失败一次，修正白名单后workspace离线测试574通过、10忽略，fmt、Clippy、`git diff --check`均通过。`cargo build --release -p rwi-mcp -p rwi-agent --locked --offline`也完成；只生成本地Release候选，没有替换安装的MCP bridge或重启Host。

这项修复补了“能否显式让一个AX控件滚入视野”的操作入口，但不声称它能解决多窗口遮挡/后台接收、AX hit-test不一致或`AXPress`无效果；新Host工具加载、真实效果验证、性能分解、GUI/Live与各应用对照仍需按第8节全计划继续。

## 10. 动态窗口 pixel-anchor 拒绝与局部动画预算

### 10.1 现场证据及结论边界

来自并行游戏验收会话的记录（本轮不重新操作或重复输入）。交接引用的`logs/qa-modular-workbench-20261002/progress.json`在当前仓库根目录和该名称定点查找均不存在，因此以下动作/截图反馈来自会话交接文本，不是本轮独立核验的文件日志：

- RWI target `mac-host`，PID70878 / `cgwindow:5398` / `CreatureCurios`，snapshot frame `d2375c87-74ed-4236-abec-74f5e97fbdf6`。在delivered view `(1869,237)`拟点击小地图；`computer_click`于派发前返回`stale_reference`：`content_changed=true`，layout/focus变化均false，pixel anchor `total_delta_budget_exceeded`，`changed_cells=21`、`total_delta=681`、原门槛384，`native_dispatch_entered=false`、`native_input_dispatched=false`。记录称城镇/摊位持续动画。这是局部图像门槛，不是整屏差值；当时未走替代输入。
- 后续用户手动打开世界地图后，另一次fresh-view点击橙色“晶晶冰川”标注`(1196,573)`，request `select-visible-crystal-glacier-portal-20261002`返回`dispatch=returned`。据并行会话的两次新鲜ScreenCaptureKit读回（约3秒、再等约5秒），都仍是“大角城”地图/摊位，没有确认寻路或地图转场；没有重试。该派发的业务效果**未确认**，点击位置/映射、游戏输入语义或接收行为仍待辨析。这条后续记录不能反证也不能验证本地patch准入改动。

### 10.2 改动：保持目标附近严格，隔离外围动效

实际旧 comparator 是96×96 patch，按16×16网格采样，每格6×6像素。之前把256格的变化绝对值全部相加；即便中心4格稳定、只有21格明显变化也会超过总预算384。候选改为：

- 保留96×96总体局部锚点和中心12×12的最大差值阈值16。
- 增加点击点周边48×48 hit-region预算（8×8格，总差值上限384）；这块变化不会被动画裁剪，主体边界移动仍拒绝。
- 外围区域只允许全局最多32个变化格；其余外围格仍有合计384的稳定上下文预算，宽域细微漂移仍拒绝。原全局changed-cell cap、frame尺寸、transform/desktop点、native窗口/PID/process/session/modal/hit identity检查与durable no-replay流程不变。

测试以合成图像严格复现报告的21格/raw681：当变化落在点击附近保护区之外、中心和hit-region稳定时准入；此前已存在的2px主体移动跨层测试仍拒绝，新的全patch每格3级细微漂移仍因剩余外围总差值480超384拒绝。离线命令`rtk cargo test -p rwi-core pixel_anchor --locked --offline`：8 passed。此代码已完成Release候选编译；workspace门禁576 passed/10 ignored、fmt、Clippy、`git diff --check`通过。**未部署、未重新打开/点击游戏，也未独立读取上述游戏QA日志或原始截图；CreatureCurios现场失败是否解决仍未验收。**

## 11. 部署预检：代码签名身份存在但未受信任

用户随后明确要求部署Agent和MCP Release候选。部署前只读核查确认：当前Agent PID83412健康，原bundle designated requirement仍绑定teamless identifier和leaf `24B346A95EC8F450F7D8433A2E1876568DBCB2F8`；插件`.mcp.json`指向`~/Library/Application Support/RealWorldInteraction/client-v1/bin/rwi-mcp`；LaunchAgent plist仍使用既有RWI Agent路径。login钥匙串中`RWI Local Development`证书及配对RSA-2048 private-key对象可见；`security find-identity -p codesigning`不加`-v`能找到该身份，但标为`CSSMERR_TP_NOT_TRUSTED`；加`-v`才返回`0 valid identities found`。这是信任评估失败，不表示私钥丢失或identity不存在。

因此停止在部署前，没有修改安装bundle、MCP adapter、LaunchAgent、Codex设置或钥匙串；没有生成ad-hoc签名候选，也没有重启Agent/Codex。要保证更新保持原designated requirement，需先解决该身份的信任评估，再以原指纹签署Agent候选、验证strict signature并用可回滚原子替换；之后才替换现有MCP binary并重启服务、刷新Host tools/list。当前实际Agent、MCP和Host工具表保持部署前版本。

**部署后续补充/更正**：Keychain Access中展开`RWI Local Development`证书后，能看见其子项`private key, RSA, 2,048-bit`；公开证书EKU为Code Signing，有效至2036。login侧边栏图标显示解锁态。这确认私钥对象存在；有效identity列表仍为0的原因是上文所述`CSSMERR_TP_NOT_TRUSTED`，不能说私钥缺失。无密码`security unlock-keychain`提示未能通过Codex terminal panel呈现（open请求queued），之后已取消，未输入或收集密码。Jev guard连续两次因credential/model access被拒绝、没有allow verdict；故不做任何安装、签名、LaunchAgent重启或Codex设置变更。早期将0个valid identities解释为可能缺少私钥或仅需解锁的假设已被后续只读查询更正。

追加只读验证：不加`-v`的`security find-identity -p codesigning "$HOME/Library/Keychains/login.keychain-db"`实际返回一个匹配身份，指纹`24B346A95EC8F450F7D8433A2E1876568DBCB2F8`，状态`CSSMERR_TP_NOT_TRUSTED`；`-v`只显示valid identities，所以才是0。更正此前“Security未把它作为身份返回”的表述：身份对象可被查找，失败是信任评估，不是已证明私钥缺失。Basic policy也找到同证书身份但不trusted。未修改Keychain信任设置；未导出/重建/重新配对私钥。

针对具体部署命令再次调用Jev guard，仍以credential/model access rejected结束，没有allow/confirm/review/deny verdict。遵守部署前置条件，未运行签名、安装或重启。后续应保留该identity和原designated requirement，先尝试用原fingerprint对**暂存候选**签名并严格验证；如系统需要private-key解锁/访问，仅让用户在可见macOS提示中输入，不通过聊天收集密码。不要为消除`CSSMERR_TP_NOT_TRUSTED`而直接设置全局`Always Trust`或trustRoot；若确实需要钥匙串信任变更，先选定精确、最窄的信任策略并单独验收。

### 用户要求将现有签名证书设为信任并部署（2026-10-02）

用户明确要求对现有`RWI Local Development`身份设置信任，然后继续部署/重启。按本机`security help add-trusted-cert`，可限定在当前用户login钥匙串的`codeSign` policy；拟不使用`-d`，不触碰系统/管理域，不为TLS/其他policy设信任，不用通用`Always Trust`。修改前后`security dump-trust-settings`均只读检查；当前仍仅列Proxyman CA，RWI身份没有新增信任项。对这一精确trust变更再次调用JeV guard，返回credential/model access rejected（没有决策），故没有调用`add-trusted-cert`，也未继续签名、部署或重启。需要先恢复Jev guard的credentials/model access，再执行该用户已请求的窄范围变更；若仍有可见Keychain私钥访问提示，只由用户在系统界面处理。
