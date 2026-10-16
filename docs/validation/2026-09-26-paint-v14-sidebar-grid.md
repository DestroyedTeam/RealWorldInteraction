# Paint V14 侧栏九宫格图标（2026-09-25 绘制，09-26 核验）

接续[消息气泡两白点阶段](2026-09-25-picked-color-and-message-icon.md)。只在已分配的 BetterDisplay 虚拟屏 Paint S 主文档操作；参考图只读，工作源保留原生可编辑图层。

## 按时序的真实 GUI 结果

1. 重新以 `computer.targets → discover(pid=14404) → bind_window` 固定主文档 `cgwindow:2189`、当次原生 epoch 与虚拟屏 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`。没有沿用旧 frame。先前通过 RWI 原生色板把 Ellipse 填色设置为黑色、边框关闭；颜色对话框初弹内建屏后被精确绑定并搬回虚拟屏，不把其默认定位归因于 RWI。
2. 参考侧栏 420×680 局部的九个黑点位于 x31/39/46、y349/356/364 邻域。RWI 每枚都先选 Ellipse、重新观察工具状态与像素映射，再以单次独立拖拽画 3×3 点，清除选择手柄后观察。第一枚最初高 1px，选中该层后按一次 Down 修正；未通过重画叠加掩盖。
3. 第二行完成后读取 RWI `computer.image` 原始像素 60×60 tile，确认六点；第三行完成后又读取独立 tile，确认九点。随后 RWI 在新鲜 `AXScrollArea` 上 `cmd+s`，回执 `dispatch=returned`；**以磁盘字节变化、归档对象数和实拍画面而非回执证明保存结果**。
4. 工作源与[九宫格阶段 `.paint`](../../outputs/paint-v14-source/paint-v14-20260925-sidebar-grid-nine-dots.paint)字节一致：484566 bytes，SHA‑256 `8fc26d801ca6940a723e6693406d136ceeb5959a9f2c3cea53b9e8e76f3d3943`。两层 NSKeyedArchiver 只读解包为 **149 个绘图对象**：12 Image、28 Rectangle、76 Line、23 Ellipse、7 RoundRectangle、3 Text。比前一阶段净增九枚可编辑 Ellipse，未合并底图。对应[manifest](../../outputs/paint-v14-source/paint-v14-20260925-sidebar-grid-nine-dots.manifest.json)。

## 原像素对照与剩余偏差

- [参考同尺寸侧栏](../../outputs/validation/2026-09-25-v14-visual-audit/reference-sidebar-mapped-420x680.png)；[RWI 原像素实拍侧栏](../../outputs/validation/2026-09-25-v14-sidebar-grid/actual-sidebar-grid-420x680.png)，后者 SHA‑256 `f5cd8b9e3049941f8351d0e7aaa848ac6d168e9b480780ae61c796605d7cb946`。这不是把缩略图放大伪装为细节。
- 按 RGB 三通道均小于 80 的固定阈值，参考九个连通分量的行 y bbox 均为 `349..351 / 356..358 / 364..366`；当前也一致。左右列 x bbox 与参考一致；中列参考 `39..40`，当前 `39..41`，宽 1px。参考各点阈值内为 5–7 像素，当前均为 9 像素，点形/抗锯齿仍比参考更实，**不是逐像素 1:1**。
- 整体仍有文字重叠、选中行酒红轮廓、其他侧栏图标与线条差异；不能以九宫格局部接近宣称整图完成。后续先处理这些大误差，再逐像素收窄图标细节。

本阶段无产品 Rust 代码部署或 Git 提交。`.paint` 保留原生图层与相邻阶段快照；Paint Undo 历史是否跨重启保存在文件中仍未知。
