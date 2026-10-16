# F05：default冷后台进入／停留／离开原生验收

## 结果

**App和Window收到3次MouseMoved；default HoverView效果仍为0，未完成修复。**

- 专用71885/cg47377、virtual4；inactive/nonkey，default NSButton firstResponder，无hover-first-responder、focus/key/click primer或global warp。
- 三条不同request/ref/lease的default move（value字段缺省）：desktop3086,562→3086,684→3086,744；Windowbase50,494→50,372→50,312，inside真实hit_region=hover_zone。
- App/Window各3次type5、invalid/drop0；dwell两次状态样本间隔1501ms。View moves/enters/exits/tooltip0、insidefalse。不是错误坐标或没投递；也不能据此断言后台hover不可能。
- public framework_pointer仍1927.5,881且在ownHover之外；FG71613及target active/key状态保持。不是物理真人连续证明。
- 26实际RWI调用、4独立mutation的原query全部terminal/released/noPending/noUncertainty/noReplay，goal_verified仍false；AX关闭/detach，child/outer0/0，ownPID不存在。
- 两次GUI前caller失败（缺owned_receiver_discovery；Node64000拒绝之后preflight过期）原exit1及输出保留，不能混算成Native错误。

新public NSEvent MouseMoved构造＋原default CGEventPostToPid只在提案／预检；尚未修改生产、启动新case或验效。Chrome coldBG-first新factory本轮DOM0，随后明确独立FG校准DOM1，两者不混算；Chrome后台仍未修复。

## 时间顺序原截图

以下直接链接已存在的原返回ImageContent逐字节解码PNG（1320×1104）；未编辑、未在本准备阶段复制到outputs，不称未经转换的原始source像素。六张原返回PNG同SHA，只能结合独立Native状态判断，不从静态图alone推断输入是否投递。

### 1．首条move前：fresh default/inactive/keyfalse，App/Window0、View0

![1．首条move前：fresh default/inactive/keyfalse，App/Window0、View0](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/cold-default-hover-entry-exit-after-f16/chronological-returned-PNG/02-call-03.png)

ledger index 3，unix_ms 1791861635718，SHA256 `0eebea60d42f1974be4399fbd7361982119b616bdf4a2d75339a36e2885bfae1`。

### 2．outside第一步后：App/Window1，View0

![2．outside第一步后：App/Window1，View0](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/cold-default-hover-entry-exit-after-f16/chronological-returned-PNG/03-call-09.png)

ledger index 9，unix_ms 1791861655781，SHA256 `0eebea60d42f1974be4399fbd7361982119b616bdf4a2d75339a36e2885bfae1`。

### 3．inside＋1501ms状态dwell后：inside hit准确，App/Window2，View/tooltip0

![3．inside＋1501ms状态dwell后：inside hit准确，App/Window2，View/tooltip0](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/cold-default-hover-entry-exit-after-f16/chronological-returned-PNG/04-call-15.png)

ledger index 15，unix_ms 1791861657874，SHA256 `0eebea60d42f1974be4399fbd7361982119b616bdf4a2d75339a36e2885bfae1`。

### 4．outside-exit后：App/Window3，View/enter/exit/tooltip0

![4．outside-exit后：App/Window3，View/enter/exit/tooltip0](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/cold-default-hover-entry-exit-after-f16/chronological-returned-PNG/06-call-22.png)

ledger index 22，unix_ms 1791861687423，SHA256 `0eebea60d42f1974be4399fbd7361982119b616bdf4a2d75339a36e2885bfae1`。

## 当前全计划（不新增要求）

|项|已确认子集|保留缺口|
|---|---|---|
|F01|已实现/构造层验过；R93字段/坐标读回、R98新factory保原source/clock并离线真编译；R99 FG及R100冷BG自有Cocoa消费|须与真实接收按层验证；构造不等于Chrome后台成功|
|F02|部分原生验收；普通AppKit点击/Unicode/Backspace/Left；R99新factory coldFG阳性、warmBG阴性；R103/104 NO/YES真实selector对照；新隔离Chrome coldBG-first DOM0、随后独立FG DOM/counter1，全10原操作终态释放，无重播|Chrome非活跃client消费修正与更多keyboard/framework/window-state；本轮coldBG已测阴性，不能再次列为未测/重播同run|
|F03|实验pointer route部分验收；明确opt-in SLEvent move/click＋AppKitfactory；cap unavailable拒绝/缺省不变；自有inactiveCocoa消费阳性；新隔离Chrome coldBG-first DOM0、随后独立FG DOM/counter1，全10原操作终态释放，无重播|Chrome非活跃client消费修正与更多keyboard/framework/window-state；本轮coldBG已测阴性，不能再次列为未测/重播同run|
|F04|普通drag部分原生验收；配对down/move/up、R19真中途cancel、R46/R51自动CommandTab、R21精确前台冲突/no replay|covered/whole-offscreen/更多客户端；普通canvas drag不算跨appDnD|
|F05|App/Window原生接收通过；default cold view/tracking有限阴性；own71885/cg47377三default move/value缺省，App及Window各3，inside native hit hover_zone；1501ms状态dwell；View/enter/exit/tooltip0，inactive/nonkey/defaultNSButton/FG71613保持，4原操作终态释放，child/outer0|对App/Window→default view/tracking差异做primary支持的受审单变量实现；拟public NSEvent Move factory、保持原CGEventPostToPid，尚未应用/启动/验效|
|F06|应用特定部分支持；Wine/GameHub自绘canvas/Live既有证据，不在本轮重复游戏|raw/relative/global-poll/captured-cursor按实际framework验证；未知非一概不支持|
|F07|焦点/保键子集原生验收；R19/R21冲突、R46自动CommandTab；R95/96/99 Receiver前台key custody，R89执行中BG→FG|非文本键/修饰键/更多窗口状态与恢复；physical human=false是证据限定，不新增真人门槛|
|F08|capture部分原生验收；covered SCK、R79 minimized原像素1320×1104、R80已有minimizedroot四帧Live|wholeoffscreen/OtherSpace/动态新鲜度；capture不赋input authority|
|F09|window-local部分原生验收；R60局部offdisplay wheel、R61 exactwindow跨屏边缘drag|wholewindow offscreen/OtherSpace/minimized input/mixedDPI/two-active seam|
|F10|minimized视觉子集验收；R79/80独立图片/帧，保留已验不再重测|formalHost minimized/hidden变化与fresh/paused语义；input/semantic另列、不自动unhide/activate|
|F11|AX部分原生验收/失败保真；动作/value独立readback；R65真NSOpenPanel modal负控制、cancel后parent可用；R66/67 family/return|更多provider/action，foreignPID OS panels尚未证；不能仅动作存在/返回即goal|
|F12|Live/task/family部分原生验收；R49任务切换、R84 formalv3两PID/multisource、R65sheetfamily、R73 280×300与Live44有限hit/28ptpaint|自动foreign ownership/原生菜单多层composition/更广family；非私有HosttaskID或1:1视觉保证|
|F13|DnD关键子集原生验收；R86两PID后台drop、R87真中途cancel/destination exit、R89 same-session执行中BG→FG外部consume|geometry/TCC变化、代表性client/payload/browser场景；不重复已验/不擦R88旧cancel|
|F14|显式内容route未整合；R91三native browser窗/incognito/lifecycle已验，非CDP/page/DOMroute|明确content-context/lifecycle/incognito route与native toolbar/OS边界；不接管用户页面|
|F15|生命周期/取消部分原生验收；durable/lease/noreplay/pairedrelease；R19/R87 cancel；R49/R84 Live隐藏/恢复/关闭/清图；R76容量回收|lost-release、disabledtap/wake、边界竞争输入；无需为睡觉用户设真人测试门槛|
|F16|格式实装/有限native画质验收；R50/R84压力/阶段时延、R94快照重复去除；F16实际MCP固定格式/真copy4相/已MCPonly安装c778；Live44有限原图/外圈hit验收；已加载真实设备native链12 snapshots的UTF8/image/capture_wall单独量测，非新formatter收益|currentHost新typed schema/model尚未验；native capture/convert/encode/transport/Agent/Hostwait拆层与有限active-stream压力，字节不是token/CPU|

## 证据与界限

- [54调用coldChrome独立审计](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/cold-background-appkit-chrome-after-f16/READ_ONLY_ACTUAL_COLD_CHROME_RECEIVING_AUDIT.json)
- [26调用F05原ledger](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/cold-default-hover-entry-exit-after-f16/ACTUAL_LOADED_RWI_NATIVE_TOOL_LEDGER.json)；[独立有限审计](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/cold-default-hover-entry-exit-after-f16/READ_ONLY_ACTUAL_F05_COLD_HOVER_RECEIVING_AUDIT.json)
- [原PNG六张manifest](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/cold-default-hover-entry-exit-after-f16/CHRONOLOGICAL_RETURNED_PNG_MANIFEST.json)
- [原关闭与进程终态](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/cold-default-hover-entry-exit-after-f16/ACTUAL_OWNED_PROCESS_CLEANUP.json)；[同四角色实际补证](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/cold-default-hover-entry-exit-after-f16/native-hover-four-role-evidence-supplement/FINAL_F05_NATIVE_EVIDENCE_WITH_OUTER_COMMAND_ROLE_REOPEN.json)

source374与生产12保护项保持；本轮没有重跑Cargo／源码copyproof／部署。既有配置1/0/1/0、copy编译0000/runtime101/0/101/0原结果保留，不把它们当hover效果。F16已安装MCPc778但currentHost新typed schema/model未验；UTF8/PNG字节及capture_wall不能称tokens/nativeCPU或普遍性能提升。全目标继续，已验子集不反复重测，也不请求人工配合。
