# Paint V14 选中行旧酒红轮廓局部遮盖（2026-09-28）

承接[搜索加号按钮176对象阶段](2026-09-28-v14-plus-button-calibration.md)。RWI 重新 `computer.targets → discover(PID 14404) → bind(cgwindow:2189)`，确认 Paint S 在 BetterDisplay 虚拟屏 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`，窗口 `[1536,24,1434,660]`，四项 macOS 权限有效。用户 GameHub/游戏仍留内建主屏，本轮没有对游戏派发输入。开工工作源 SHA‑256 `485c6eed0a0a40cc3381a1d7516071f9d4defd59600ddddd4e7b430abc17a143`。

## 先确认对象边界

[原参考](../../outputs/validation/2026-09-28-v14-selected-row-ring/00-reference-420x680.png)与[开工176对象原生截图](../../outputs/validation/2026-09-28-v14-selected-row-ring/01-before-176-objects.png)显示第5行头像外有明显酒红圆环及上下横线。只读解包 `.paint` 双层 NSKeyedArchiver：最底层第0个 DrawImage2 为2000×679整图，**酒红双环在这张旧底图里**；第132个 DrawImage2 为48×48头像，位置对应截图 x78–126/y422–470；本行列表文字并非独立 DrawText。不能直接删除不存在的酒红线图层，也不能用整幅截图单层覆盖来冒充分层复刻。

## 按顺序的原生操作与验收

1. 用 Paint GUI 的 Rectangle 工具保留 `#F2F2F2` 填充、无边框，在头像左外侧画 x64–72/y405–485 的窄矩形。随后以输入颜色 `#DBDBDB`（回读 `rgb 0.858824…`）逐块遮盖左内侧、顶部、右侧及底部旧环，**没有覆盖文字区域 x≥142**。[六块遮盖后原生截图](../../outputs/validation/2026-09-28-v14-selected-row-ring/02-six-masks-before-final-pixel-strip.png)。输入颜色对话框默认出现在内建主屏，RWI精确发现/绑定后立即移到虚拟屏并稳定读回；`Input Color Manually` 的 AXPress 返回 `-25204/uncertain`，查询原请求且只依据独立发现的真实对话框继续，未重放。
2. 还剩右边界 x140–141 的酒红像素，追加一块2px宽独立 Rectangle 后，头像框**外**的酒红阈值像素由569降至0。[七块遮盖并保存的183对象阶段截图](../../outputs/validation/2026-09-28-v14-selected-row-ring/03-final-saved-183-objects.png)；该阶段可回退源在 `outputs/paint-v14-source/paint-v14-20260928-selected-row-ring-masked.paint`，SHA‑256 `b09240e1b7fd2b0343b5ab3612d7cc37ef3664ba5283edc1b2a8785e3f9c7ea9`。
3. 4倍局部对照发现头像顶边仍透出旧底图酒红线。RWI再画一条15×1px `#DBDBDB` 填充 Rectangle，**实测只改变 y422/x95–109 这15个像素**，局部均方误差19272.8→671.7。[184对象阶段截图](../../outputs/validation/2026-09-28-v14-selected-row-ring/04-final-saved-184-objects.png)，源快照 `outputs/paint-v14-source/paint-v14-20260928-selected-row-ring-and-avatar-edge.paint`，SHA‑256 `96b8ba0e205ab1b74f34426a45fb0f4533279b9818a9708723d99f47b9e7a663`。
4. 最后放大检查仍有 x78–79/y422 的2个酒红像素，再以独立2×1px Rectangle遮盖。只读原像素证明确实**仅改变这2像素**，且对应参考灰值从均方误差28968.8降至0。`cmd+s`后重新取[最终185对象原生截图](../../outputs/validation/2026-09-28-v14-selected-row-ring/05-final-saved-185-objects.png)，与保存前截图逐字节一致。[参考左／实际右4倍局部](../../outputs/validation/2026-09-28-v14-selected-row-ring/reference-left-actual-right-final-185-4x.png)。所有正式截图均由 RWI `computer.image` 对 `2868×1320` 单窗原始artifact裁 `[1000,390,420,680]`，非1024px预览放大；4倍图仅用于肉眼比较。

固定 x60–145/y405–485 区域 RGB 均方误差 **4881.3→2682.4（降45.0%）**；按 `R<210, R>G+30, B>G+15, G<160` 的限定酒红阈值，头像框外569→0像素，框内57→40（头像本身的紫/红色不能一概当作旧环）；右侧文字区 x142–350/y420–465 **0个像素被改动**。这是局部真实改进，不是选中行或整页1:1；现仍可见左端非目标的矩形灰底/圆角差异、头像图像位置及文字偏移/重影。

正式工作源 `work/replica-verify-20260915/run6/paint-v14-card-cleanup-20260919.paint` 与[最终可编辑源](../../outputs/paint-v14-source/paint-v14-20260928-selected-row-ring-masked-final.paint)字节相同：490905 bytes、SHA‑256 `85f1f52473a4e307822784c3e6fa336af3287a8d026fbdfac308e8304648a52c`；双层归档核对 **185对象**（12 Image/39 Rectangle/99 Line/23 Ellipse/9 RoundRectangle/3 Text）。新增9块矩形均可独立编辑；原头像 Image 与文字保留。[manifest](../../outputs/paint-v14-source/paint-v14-20260928-selected-row-ring-masked-final.manifest.json)。下一步不再无限叠小色块，应优先处理选中行左端圆角与列表文字的结构性偏差；由于文字大多烙在底图，须局部遮盖后用真正 DrawText 重建，并逐行核对字体/位置。

本轮没有 Rust 产品代码改动、部署、Git 提交或推送；不冒称重跑 cargo 门禁。`rtk git diff --check` 单独核对。
