# RWI 字号组合框修复与 Paint V14 第5行可编辑标题（2026-09-30）

承接[187对象选中行边缘阶段](2026-09-29-v14-selected-row-edge-and-text-probe.md)，开工正式 `.paint` SHA‑256 `1066a6a78d7c69716ff199e67776a0f47cf2e6439391b316af1b7ee04088cc57`。Paint S PID14404、`cgwindow:2189` 继续在 BetterDisplay 虚拟屏 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`；GameHub/用户游戏留在内建主屏，未对游戏派发输入。

## 产品修复、部署、Host验收

1. CodeGraph定位 `crates/rwi-macos/src/native.rs` 的 `TextClass::classify` 将 `AXComboBox` 归为非文字，故字体大小框虽 AX回读可见却被 `computer.type_text` 在派发前拒绝。加入 `AXComboBox` 的非安全文字分类；原有**精确焦点native对象相等、可写AXValue、非安全subrole**三道后续检查均保留，不因显示了字符就授权其他控件。对应离线单测覆盖 ComboBox 已知/未知/安全subrole。`cargo fmt --all -- --check`、macOS crate **88 passed/6 ignored**、全 workspace **525 passed/10 ignored**、全目标全特性Clippy `-D warnings`、`git diff --check` 均通过。
2. Release Agent以既有本地 `RWI Local Development` 签名（leaf `24B346A95EC8F450F7D8433A2E1876568DBCB2F8`），阶段副本严格codesign验证后，仅备份并替换原 `~/Library/Application Support/RealWorldInteraction/agent-mac-v1/RWI Agent.app`；配置、配对、journal、MCP/Plugin、其他应用均未改。已安装可执行文件 SHA‑256 `5d4514a29774a465291d0ec62851eed810521a349e7bf4eecc9869e8bb5f0072`，LaunchAgent重新启动后 PID19786/实例 `7f9c29e2-f734-4b26-aefe-daad9ce9e862`，四项TCC权限仍true、input fence=false、Host MCP连接healthy。旧 `.app` 私有备份在 `work/rwi-text-combo-deploy-20260930/RWI Agent.before.app`，未提交。
3. 真实Paint Text工具中，对字号AXComboBox精确点击、`cmd+a`、`computer.type_text("16")`，派发返回且AX回读16；`AXConfirm`后仍为16。后续同路径改17并回读17，证明插件重新加载的实际Agent代码生效；没有把离线测试当Host验收。
4. Text颜色的直接色板仍是另一个问题：独立CG `cgwindow:12733` 可捕获但AX匹配本次仍 `visual_only=true`，未对只读图像推断输入。改用RWI Eyedropper从Paint标题实测深色像素取 `#050505`，Picked Color面板首次落主屏，立即精确绑定并移至虚拟屏，稳定读回；它实际提供 `Set as Text Color`，AXPress后主窗文字色井回读 `rgb 0.019…`。同法从纯平背景取 `#DBDBDB`，AXPress `Set as Fill Color` 后矩形填色回读 `rgb 0.860291…`。这是一条已验收的替代取色路径，**不代表直接文字色板的AX缺口已修复**。

## 复刻过程及真实截图

1. [开工187对象原生截图](../../outputs/validation/2026-09-30-v14-row5-title/00-before-187-objects.png)：参考第5行标题暗像素bbox `(146,425)..(242,439)`，旧烙底标题 `(142,426)..(237,440)`。用Paint无边框、`#DBDBDB`可编辑Rectangle，仅遮x139–247/y422–445的旧标题，保留头像、时间与下方消息预览。
2. 初试字号16，生成独立Text“拉电线了诶嘿”并[原像素留证](../../outputs/validation/2026-09-30-v14-row5-title/01-size16-trial.png)：暗bbox `(149,428)..(241,441)`，固定区RGB MSE 5358.9→4846.3，偏右/偏下且偏小；RWI Undo后确认旧标题仍被遮，试验Text未保存。
3. 字号17、前一位置[试验截图](../../outputs/validation/2026-09-30-v14-row5-title/02-size17-y451-trial.png)改善至MSE3130.8，暗bbox `(146,426)..(244,439)`；再Undo并上移1像素，以Text工具输入同样六个汉字，Escape结束编辑、切回单行Rectangle工具去掉编辑手柄。[最后一次未保存试验](../../outputs/validation/2026-09-30-v14-row5-title/03-size17-y450-trial.png)暗bbox `(146,425)..(244,438)`。固定 x135–260/y420–448 RGB MSE **5358.86→2833.41**（下降47.1%）；参考右沿242而当前244、参考下沿439而当前438，字体/灰阶仍非逐像素等同。某次误复用字号输入的request_id给Text内容，原生返回 `operation_payload_conflict` 且新内容未派发；查原请求后以新稳定ID发起不同的新意图，没有重放原请求。
4. RWI `cmd+s` 后[189对象最终原生截图](../../outputs/validation/2026-09-30-v14-row5-title/04-final-saved-189-objects.png)与试验图可见内容相同（截图颜色归一化每通道最多2级抖动）；[参考／改前／改后3倍对比](../../outputs/validation/2026-09-30-v14-row5-title/reference-before-after-title-3x.png)。所有正式420×680图均由RWI `computer.image` 从2868×1320单窗原始artifact裁 `[1000,390,420,680]`，不是低清预览放大。

正式工作源与[本阶段可编辑 `.paint`](../../outputs/paint-v14-source/paint-v14-20260930-row5-title-editable.paint)同为492495 bytes、SHA‑256 `12d28a4b39e3ff2c06d82f4940d3c4685e9009cc62310496df1ec638f8f295e7`；双层NSKeyedArchiver只读验 **189对象**（12 Image/42 Rectangle/99 Line/23 Ellipse/9 RoundRectangle/**4 Text**）。新增的DrawText持有原生可编辑字符串“拉电线了诶嘿”、17pt系统字体和深灰色属性，不是截图贴片。[manifest](../../outputs/paint-v14-source/paint-v14-20260930-row5-title-editable.manifest.json)。旧版187对象与全部试验截图保留作回退/负例。

下一步继续处理第5行消息预览及其他列表行烙底文字，先按参考尺寸/颜色建立原生Text工作流；仍需解决文字色板AX只读和跨屏Edit菜单 `visual_display_unknown`，并避免整图栅格遮盖。选中行左灰底、头像位置和其他页内容仍有明显差异；**整页不是1:1**。无Git提交/推送。
