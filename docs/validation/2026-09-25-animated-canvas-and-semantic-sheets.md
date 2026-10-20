# 动画选框与无像素 AX 弹窗实机验收（2026-09-25）

## 目标与隔离

用户重启 Codex 后，本任务新 MCP schema 的 `computer.targets({device_id:"mac-host"})` 实测约 **25ms**，只返回 mac-host；四项 TCC 均 true。此后 RWI GUI 操作均走当前任务加载的 MCP。为避免破坏 V14 正式源，将已保存 `.paint` 复制到 `work/paint-v14-semantic-dialog-test-20260925.paint`（初始 SHA-256 与正式源同为 `1573f4f74b45a6c13014dd9c0efeb1d2d3a2185f9fa4368edab74c91ba8a9998`），用 Paint S 后台打开。新窗口最初落在内建屏，RWI 对其精确发现/绑定后，`computer.place_window` 原生移至 BetterDisplay UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`；重新发现的 bounds 为 `[1536,24,1220,527]`，没有把旧 epoch 用于输入。正式窗口仍为 `cgwindow:2189`。

## 按时间顺序的实际结果

1. RWI 2048px 快照原图为 2440×1054。用图像坐标选中测试副本里的企鹅头像后，独立快照确认图层 `Width:48 Height:48`。第一次选取曾在预派发阶段报 `stale_reference`，耐久回执显示 `content_changed=true`、`native_input_dispatched=false`，未重放该请求；新观察后的独立动作成功选取。
2. 选框动画使整窗内容哈希持续变化。两张按时间先后取得的真实 ScreenCaptureKit PNG：[动画帧 A](../../outputs/validation/2026-09-25-animated-canvas/before-a.png)、[动画帧 B](../../outputs/validation/2026-09-25-animated-canvas/before-b.png)，均为 2440×1054。逐像素分析仅 **135 / 2,571,760** 个像素变化，集中在头像选择框；150×150 的局部区变化 135/22500，而远端聊天头部/空白区未变。旧整窗哈希门槛仍拒绝一次调整手柄的拖拽，耐久回执同样证明没有原生输入。原图仅留本地验收输出，不纳入 Git。
3. 修订后的 Mac 专用像素路径只对有纹理、离窗口边缘至少 64px 的 96×96 原图局部建立有界 16×16 特征；仅接受小量选框动画。窗口/PID/进程出生、几何、显示器映射、焦点/模态、租约、耐久 request ID 和原生 AX 命中检查仍生效。**只在命中精确 `AXScrollArea` 画布时允许这项宽容**；其他按钮、空白区域和未 opt-in 的 Windows/fixture 仍用严格整窗门槛。单元/跨层回归覆盖动画可过、目标移动 2px 拒绝、空白/损坏 PNG 拒绝、未 opt-in 后端拒绝、派发前再次检查。没有自动重放输入。
4. 在隔离副本上，新版 RWI 手柄拖拽返回 `dispatch=returned`；独立 Paint 快照从 `48×48` 读到 **`51×58`**，是真实改变而非仅 API 回执。提示框首次呈现时，`semantic_dialog_probe` 因动画中的 CG 几何不一致暂报不完整；稳定后的精确 PID 发现返回无像素逻辑 `AXDialog cgwindow:5275`，另有不同 ID 的渲染面 `cgwindow:5276`。提示框**落在内建屏**，这是仍待解决的应用/弹窗放置问题。绑定逻辑 ID 后 `snapshot` 的 `visual_available=false`、`view_id=null`，AX 控件含提示文字、未勾选的“Do not show this message again”和 `OK`；经原生 `AXPress` 点 `OK` 后再次发现提示消失。未借渲染 ID 的像素定位或输入。
5. 关闭已修改的测试副本时，RWI 向精确聚焦画布发 `cmd+w`；随后出现**虚拟屏**上的“是否保存”面板。可视 CG ID `5615` 不等于 `AXFocusedWindow` 的逻辑 ID `5614`。后者 `AXRole=AXSheet`、`AXSubrole`/`AXModal` 未报告，且应用级中心 hit-test 错落到另一主文档 `cgwindow:2189`。旧逻辑因此正确拒绝把 CG5615 当作它，但也无法触达该真实 Sheet。
6. 新版仅对**精确聚焦、与 AXMainWindow 不同的 AXSheet role**放行直接 AX 语义路径；仍要求逻辑 Sheet 自己的 CG all-window ID/PID/几何一致、前后焦点/进程身份不变，只能执行其明确 advertised 的 AX action。普通未知 AXWindow 不获此例外。部署后重新发现 `semantic_only cgwindow:5614`，快照 `visual_available=false`，明确列出 `Save`、`Don’t Save`、`Cancel`。RWI 对**测试副本**的 `Don’t Save` 原生 AXPress 返回，独立发现随后确认 Sheet 与测试副本窗口均消失；正式 V14 窗口和参考窗口仍在。此操作仅丢弃测试副本的未保存改动，磁盘副本与两份正式 `.paint` SHA 仍不变。

## 回归、部署和剩余工作

- 最终 `cargo fmt --all -- --check`、`cargo test --workspace --locked --offline` **514 passed / 10 ignored**、全 targets/features `clippy -D warnings`、`git diff --check`、Agent release 构建通过。真实 Host 在各次 Agent 更新后重新连接，四项 TCC 仍 true；原本地代码签名 designated requirement、Agent 配置及 client profile 未变。最终安装 Agent SHA-256 `a0770bc7ebca294a3d681a58e206d96676e7e300dcc4b3415bff012c0ca243b7`；MCP SHA-256 `551c37f09232a491244f3bdcfdc6580abfadf014ed4e6c6abcd7e54f136294e4`。Windows VM 未唤醒。
- 对 mac-host 的 8 次连续 2048px 快照，**确认任务面板可见时** Live 一直可见，帧数由 127 增至 131；另一次 Live 隐藏是任务面板实际不可见，重新导航当前任务后恢复，不能误归因为读操作争用。这只是读操作样本，不是长输入期间的完成验收。
- Paint S 提示框仍可能被应用放到内建屏；逻辑语义操作已可完成，但跨显示器放置/遮挡策略需继续验证。正式 V14 内容本轮没有绘制或保存，跨重启 Undo 历史仍未证明。未提交/推送，未触发私有仓库 Actions。
