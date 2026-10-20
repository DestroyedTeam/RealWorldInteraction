# V14 第5–6行文字与弹窗焦点 owner（2026-09-30）

承接[189对象阶段](2026-09-30-v14-row5-editable-title-and-combo.md)。Paint S PID14404 / CG2189 仍在 BetterDisplay虚拟屏 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`。未移动、激活或输入 GameHub/游戏；只读前台元数据与实际GUI验收分开记录。

## 按时间顺序的实际过程

1. [189对象开工原生图](../../outputs/validation/2026-09-30-v14-row5-preview/00-before-189-objects.png)。RWI Eyedropper→Picked Color读灰色 `#818181`，面板先落主屏、精确绑定后移到虚拟屏。Set as Text Color会切为Text工具：第一次沿用取色前工具状态的拖拽只进入标题编辑覆盖层；重选Rectangle后确认标题未移，未盲目Undo原标题。
2. 重读Rectangle/Fill=1/Border=0，用独立 `#DBDBDB` Rectangle遮第5行旧预览 x139–225/y448–470。14号[试字](../../outputs/validation/2026-09-30-v14-row5-preview/03-preview14-trial.png)偏窄、误差变大，Undo后改15号 `wu.:  [图片]`。[191对象保存图](../../outputs/validation/2026-09-30-v14-row5-preview/05-saved-191-objects.png)。
3. 实测第6行底色为 `#F2F2F2`。用两个独立Rectangle分别遮旧标题 x139–303/y497–521、旧预览 x139–325/y524–546；头像、时间、静音图标不遮。[中间遮盖图](../../outputs/validation/2026-09-30-v14-row5-preview/06-r6-preview-covered.png)含编辑手柄，不是作品完成图。
4. 第6行预览15号太宽、14号太窄，均Undo。字号框实际接受并回读14.5；以原生Text `VincentAdamNemessis:  …` 创建并下移1px。[最后预览试验](../../outputs/validation/2026-09-30-v14-row5-preview/10-r6-preview14p5-y525-final.png)。Paint曾把输入的三个句点自动改成单个省略号，按AX回读保留，不声称输入/回读字节一致。
5. 细笔画取色曾回读 `#929292`/`#5D5D5D`，不能把截图中的单个深色像素当作取色回读保证；后者未设置。改从侧栏宽深色区域拾取 `#141414`。一度CG on-screen发现只列主文档/参考窗，但主窗 `blocked=true`、`computer.result /popups`仍含Picked Color CG9763。Computer Use的Paint AX状态实际读到该面板及 `Hex:#141414`，经其Set as Text Color/Close恢复主文档。RWI独立回读文字色井 `rgb 0.0772627…`、blocked=false。**两次CUA处理是对照/恢复，不冒充RWI输入验收**；CG列表当时未包含该面板的原生原因尚未确认，不能把有界发现当完整缺失。
6. 第6行标题原生Text `MacApp 交流讨论…`，16号偏窄/偏上，Undo后16.5号改善。RWI重新聚焦画布、cmd+s，独立读盘验源。[195对象正式保存图](../../outputs/validation/2026-09-30-v14-row5-preview/13-saved-195-objects.png)；[三倍对照：左到右参考/189对象/195对象](../../outputs/validation/2026-09-30-v14-row5-preview/reference-before-after-rows5-6-3x.png)。420×680正式图均是RWI从2868×1320原始artifact裁 `[1000,390,420,680]`，不是低清预览放大。

## 原像素与可编辑源

固定ROI为半开区间，bbox包含端点；灰字阈值all RGB<180、标题<100。

| 区域 | ROI x1/y1/x2/y2 | RGB MSE 189对象→195对象 | 降幅 | 参考bbox→实际bbox |
|---|---|---|---|---|
| 第5行预览 | 135/448/235/473 | 565.1008→226.4488 | 59.9% | 146/452/219/465→146/452/220/466 |
| 第6行标题 | 135/495/307/523 | 6263.7115→1961.2904 | 68.7% | 147/500/288/515→147/500/287/515 |
| 第6行预览 | 135/522/327/547 | 1287.6344→489.8550 | 62.0% | 146/528/320/537→146/528/319/538 |

- [191对象源](../../outputs/paint-v14-source/paint-v14-20260930-row5-preview-editable.paint)：493763 bytes、SHA256 `0fdc2781373128a0a330db0c0d10a7e116a9c09409b28798b8304374be1c5055`。
- 正式工作源与[195对象源](../../outputs/paint-v14-source/paint-v14-20260930-row6-text-editable.paint)字节相同：495923 bytes、SHA256 `1c83bef6cc429724a9c23dff501c1a6de795c486ce7f15af0273447f9d9b1f49`。双层NSKeyedArchiver只读核对 **12 Image/45 Rectangle/99 Line/23 Ellipse/9 RoundRectangle/7 Text**；新增文字仍是原生NSTextStorage。保留各阶段源和真实截图；跨应用重启的逐操作Undo历史持久性仍未知。[manifest](../../outputs/paint-v14-source/paint-v14-20260930-row6-text-editable.manifest.json)。
- 仍有1px边沿/字体灰阶差，第6行时间与参考不同，其他列表字/头像/旧輪廓等未完成，**整页不是1:1**。

## RWI修订与实际验收边界

`native/semantic.rs` 原本只从AXFocusedWindow探查逻辑dialog。新增候选路径：该属性仍指主窗时，核对精确 **AXFocusedUIElement→AXWindow** 的不同dialog ID。要求应用/控件/owner PID一致、两个非零不同窗口号、dialog分类、进程出生、独立CG all-window自身ID/PID/几何，以及前后CF对象/焦点/几何一致。没有按标题、同PID、近似位置猜owner；语义-only仍无像素授权，只允许advertised AX action。

新增两项离线负向测试，拒绝普通窗口、相同/零/未知native ID。macOS crate **90 passed/6 ignored**；fmt、workspace **527 passed/10 ignored（41 suites）**、全targets/features Clippy `-D warnings`、release构建通过。沿原本地证书leaf `24B346A95EC8F450F7D8433A2E1876568DBCB2F8` 签名、严格校验，仅替换/重启RWI Agent；安装executable SHA256 `b5e467cf77d4b194699e3ed59ed30998d8eb28d14ad1046ac93385bcd0455aa5`，PID81531 / instance `d73a3ffc-9b74-4936-bda0-41d54c9b68fb`，四项TCC仍true、MCP healthy。配对/config/journal/Plugin/MCP与其他应用未替换，不需重启Codex。旧app备份在 `work/rwi-focused-owner-deploy-20260930/RWI Agent.before.app`。

部署后RWI实际取色、读 `#141414` 并移到虚拟屏，独立读回 `[1536,24,341,367]`、rendered_surface_confirmed=true。[部署后面板图](../../outputs/validation/2026-09-30-v14-row5-preview/14-postdeploy-picked-color-virtual.png)。用户切回另一应用后，独立前台元数据确认Wine PID33623；[此时面板原图](../../outputs/validation/2026-09-30-v14-row5-preview/15-picker-after-game-switch.png)仍可读，RWI AXPress Set as Text Color/Close后主窗blocked=false、色井正确，再发现确认CG9763消失，**Wine仍前台，正式画稿SHA未变**。随后另一取色创建的首帧因动画几何不一致报 `semantic_window_mismatch` / probe complete=false，稳定再读complete=true；没有沿用动画中的epoch输入，面板移回虚拟屏并关闭。

以上实机结果仍是正常可见CG路径，**仅AX可见/焦点错位的新候选分支尚未再次实机触发，不能报告全修复**。下一阶段继续收集该条件并验收，不扩大输入权限。补充调研复核既有取色/后台输入/语义Sheet记录、冻结CrossMacro清单和本地CUA参考资料。Apple [CGEvent概述](https://developer.apple.com/documentation/coregraphics/cgevent)说明事件最终由目标进程的事件处理机制接收；它不是应用目标成功的原生确认，此解释与实际回执/效果证据分开。无Git提交/推送。

[部署验收元数据](../../outputs/validation/2026-09-30-v14-row5-preview/postdeploy-acceptance.json)及[118项MCP动作回执](../../outputs/validation/2026-09-30-v14-row5-preview/mcp-operations.json)保留本地。辅助 `jev_review_completion` 已尝试，但凭据/模型访问被工具拒绝，未获得完成判定；本轮只报告阶段进展，不报告总体完成。末尾再次确认取色面板已消失，部署前后第5–6行截图每通道最大差仅1级，正式源hash不变；最终fmt/diff check通过。
