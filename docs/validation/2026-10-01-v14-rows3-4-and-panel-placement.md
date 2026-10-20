# Paint V14 第3–4行文字与弹窗放置

2026-09-30 开始，2026-10-01 完成本阶段保存。Paint S 位于已分配的 BetterDisplay 虚拟屏；本阶段未向 GameHub/游戏发送输入或移动其窗口。所有保留的绘图/文字/保存操作来自加载的 RWI MCP。Computer Use 只读对照了 Paint 的 AX 状态和截图；Rust 的显式 PID 诊断只读 AX/CG 元数据，Python/FFmpeg 只做离线归档审计和像素比较。

当前[215对象可编辑源](../../outputs/paint-v14-source/paint-v14-20260930-rows3-4-editable.paint)与工作源逐字节相同：503094 bytes，SHA256 `b0fd831fc630ba33b2f363b992c1c429b810f3f978f52116f54474673b8d1995`。分类为12 Image、55 Rectangle、99 Line、23 Ellipse、9 RoundRectangle、17 Text。新增6个原生Text、6个局部Rectangle；临时取色线未保存，Line数仍为99。[manifest](../../outputs/paint-v14-source/paint-v14-20260930-rows3-4-editable.manifest.json)含归档实际字符串、字体、字号和指标。Undo历史跨应用重启是否持久化仍未知；独立对象和阶段源均保留。

## 实际截图与结果（按操作顺序）

1. [203对象阶段](../../outputs/validation/2026-09-30-v14-rows3-4/00-before-203-objects.png)：第3–4行文字偏左，头像背景块和旧轮廓仍明显。取背景为`#E6E6E6`，局部遮旧字，再建原生Text。第4行标题[16.5号试验](../../outputs/validation/2026-09-30-v14-rows3-4/01-row4-title16p5.png)降低误差，但微拖落在缩放柄，造成[裁短](../../outputs/validation/2026-09-30-v14-rows3-4/02-row4-title-aligned.png)。第一次Undo请求在发事件前过期，查原请求后用新帧撤销，画面[逐像素恢复](../../outputs/validation/2026-09-30-v14-rows3-4/04-row4-resize-restored.png)，不是用成功回执推断恢复。
2. [第4行17号最终标题](../../outputs/validation/2026-09-30-v14-rows3-4/06-row4-title17-final.png)和[第3行17号标题](../../outputs/validation/2026-09-30-v14-rows3-4/07-row3-title17.png)保留；字号和字符串在`.paint`中验证。标题色实际取到`#010101`，不是先前细字边缘取到的混合色。第3行标题暗区bbox与参考一致。
3. [两行旧预览已遮盖](../../outputs/validation/2026-09-30-v14-rows3-4/08-previews-masked.png)。临时宽灰线取到`#7F7F7F`，设置为文字色后Undo；空白区原生图确认线已清除。第3行新增14.5号“[图片]”，第4行新增“本分牛: 这群人真是太逗了”。[14.5号结果](../../outputs/validation/2026-09-30-v14-rows3-4/12-row4-preview14p5.png)仍短3px且高1px；改为[14.8号](../../outputs/validation/2026-09-30-v14-rows3-4/16-row4-preview14p8-trial.png)后，用精确Canvas键盘引用Down下移1px，[结果](../../outputs/validation/2026-09-30-v14-rows3-4/17-row4-preview-nudged.png)bbox与参考一致。出现过蓝色编辑覆盖层，最后Select and Move→空白区点击清除；最终截图不包含该覆盖层。
4. 取色面板一次被[自身文档盖住](../../outputs/validation/2026-09-30-v14-rows3-4/09-gray-dialog-obscured.png)。只读Rust诊断确认：面板9763的`AXModal=true`，中心AX命中2189文档，CG顺序也在文档后面；原`blocked=true`正确。问题是放置函数总选显示器左上角，与文档重合。修改为真正可视dialog优先选同PID窗口旁空位；普通窗和semantic-only逻辑窗保持原策略，无空位时保留原位置规则，仍需原生hit/modal检查。部署后面板从`[1536,24,341,367]`移到`[2994,24,341,367]`，[控件重新可用](../../outputs/validation/2026-09-30-v14-rows3-4/10-gray-dialog-free-space.png)，`blocked=false/mutation_ready=true`，RWI实际设色并关闭。
5. [遮盖两处旧日期](../../outputs/validation/2026-09-30-v14-rows3-4/13-times-masked.png)。一次拖拽返回但未遮掉文字，随后以新观察明确聚焦空白Canvas、选Rectangle，重新作独立操作并验证遮盖；最终归档只新增预期6个Rectangle，没有多余图形。字号框聚焦一次AX读取超时，原请求已查，零按键/按钮、native writer idle的独立原生证明后显式解除围栏，旧请求未重放。
6. 日期落点是近乎纯色区域，不能签发有纹理的局部像素证据；字号框Caret闪烁导致严格全窗哈希拒绝，均在发事件前停止。两张完整原生图差异只有字号框138px，落点周围无变化。结束编辑、重选工具、重新读取稳定帧后成功创建12号“星期日”和“01/12”；单次Escape会改变工具，不能继续沿用原toolbar映射。第3行日期再[下移1px](../../outputs/validation/2026-09-30-v14-rows3-4/20-weekday-nudge-trial.png)，bbox与参考相同。这里保留了纯色区全窗防线，没有因卡顿放宽校验。
7. RWI `cmd+s` 后[最终215对象截图](../../outputs/validation/2026-09-30-v14-rows3-4/21-final-215-objects.png)、[完整2000×679原生画布](../../outputs/validation/2026-09-30-v14-rows3-4/22-final-full-canvas.png)。[3倍对照](../../outputs/validation/2026-09-30-v14-rows3-4/reference-before-after-rows3-4-3x.png)依次为参考、203对象、215对象；比较图由真实原像素拼接，未改动GUI作品。第1–2行、头像背景块、旧轮廓、其他图标和右侧布局仍待修，整页未达到1:1。

## 固定ROI结果

| 区域 | 203对象 MSE | 215对象 MSE | 降低 |
|---|---:|---:|---:|
| 第3行标题 | 1900.57 | 22.20 | 98.83% |
| 第3行预览 | 518.94 | 255.95 | 50.68% |
| 第3行日期 | 339.53 | 79.27 | 76.65% |
| 第4行标题 | 2603.59 | 150.40 | 94.22% |
| 第4行预览 | 1063.20 | 563.26 | 47.02% |
| 第4行日期 | 298.62 | 41.59 | 86.07% |

字体/抗锯齿仍有差异；bbox一致不等于逐像素一致。第4行标题最优MSE仍有左/上1px差异，机械对齐bbox的试验反而变差，保留误差更低的候选。

## 部署与验证

全仓离线529 passed/10 ignored，macOS crate 92 passed/6 ignored，fmt check、全目标/全特性Clippy `-D warnings`、release Agent构建及diff check通过。仅RWI Agent沿用同一签名leaf `24B346A95EC8F450F7D8433A2E1876568DBCB2F8`部署；严格签名验证通过，已安装可执行文件SHA256 `e33c39ddfd461f67b4cf2c3f1e0262d6b7f60d10cbb5c1f0c45d3589e77d231e`。PID86740 / instance `2705483a-f642-4111-a5f0-b1ac8b466ae8`，四权限仍true，MCP新target自动恢复；无需重启Codex。旧Agent备份在`work/rwi-panel-space-deploy-20260930/`。未Git提交/推送。

原生接口参考：[Apple AXFocusedUIElement](https://developer.apple.com/documentation/applicationservices/kaxfocuseduielementattribute)、[Apple AXFocused](https://developer.apple.com/documentation/applicationservices/kaxfocusedattribute)、[Apple SCScreenshotManager](https://developer.apple.com/documentation/screencapturekit/scscreenshotmanager)。它们分别描述焦点对象/属性与单帧捕获；本文的遮挡、Caret变化和实际应用效果均来自上述本机证据，不由API文档推断。
