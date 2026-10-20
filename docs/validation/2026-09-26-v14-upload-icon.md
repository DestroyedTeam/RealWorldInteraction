# Paint V14 第 4 枚侧栏图标：上箭头方框（2026-09-26）

接续[149 对象九宫格阶段](2026-09-26-paint-v14-sidebar-grid.md)和[未保存试笔记录](2026-09-26-upload-icon-trial-pending.md)。只在已分配 BetterDisplay 虚拟屏的 Paint S 正式文档操作；本轮起始磁盘源仍为149对象、SHA‑256 `8fc26d801ca6940a723e6693406d136ceeb5959a9f2c3cea53b9e8e76f3d3943`。

## 按时序的实际 GUI 结果

1. Codex 重启后重新 `targets→discover(pid=14404)→bind(cgwindow:2189)`。RWI原像素 tile 中，上一轮大黑方框内部的网格样线仍在；读 Codex Computer Use **只读 AX 状态**才看到 Paint 原生 `Edit → End Editing`。RWI 主窗重新快照时也实际列出该 AXMenuItem，遂用 RWI `AXPress` 结束编辑。独立原像素 tile 由大网格变为一枚小**圆形**：证明网格主要是编辑覆盖层、实体图层却因默认 `Corner X/Y=10` 被圆化，不是目标方框。RWI AX Undo 后原像素恢复空白，未触碰此前九宫格。Computer Use 本轮没有发任何 Paint 键鼠输入。
2. RWI将 Round Rectangle 的 Corner X/Y 用原生数值文本框改为3；第一次 `cmd+a` 回执虽 returned，文本实际变成 `130`，证明回执不等于选择文本成功。改用 RWI **double-click 文本框→type_text**，两框独立回读 `3`，Enter 后重验。描边黑色、Border on/Fill off。第一版描边宽2，加一条箭杆/两条斜线/横线，共5对象；实拍与参考阈值80暗像素为230对127，外 bbox 还大约1px，明显偏粗。RWI连续5次按各自新请求 Undo，每次均 returned；原像素确认该尝试全部消失，九宫格保留。
3. 第二版把方框描边设为1、圆角3，拖拽范围向下校准1px；黑色暗像素 bbox 与参考同为 `(30,254)..(49,274)`。Line宽2绘制独立箭杆和左右斜线；原像素发现箭头中段空隙，再以宽1独立横线与短连接线局部修正。每枚新形状都用 RWI 精确打开 `Edit → End Editing` 清除选择覆盖层，随后取新的原像素画面；没有把选择控制柄当作成品。最终新增 **1 RoundRectangle + 5 Line** 六个可编辑对象。
4. RWI在新鲜 Paint AXScrollArea 上 `cmd+s`。正式[可编辑 `.paint` 阶段源](../../outputs/paint-v14-source/paint-v14-20260926-sidebar-upload-icon.paint)与工作源字节一致：**155对象、485659 bytes、SHA‑256 `a38ceac4c7e0306d378354f71ad34fd55044382a92c88c02aa1b1ee89f11af7b`**，只读 NSKeyedArchiver 为12 Image/28 Rectangle/81 Line/23 Ellipse/8 RoundRectangle/3 Text。此前149对象快照没有覆盖。见[manifest](../../outputs/paint-v14-source/paint-v14-20260926-sidebar-upload-icon.manifest.json)。

## 同尺寸/原像素比较与后续

- [参考420×680](../../outputs/validation/2026-09-25-v14-visual-audit/reference-sidebar-mapped-420x680.png)；[最终 RWI 原生420×680](../../outputs/validation/2026-09-26-v14-upload-icon/actual-sidebar-420x680.png)；[实际70×70原像素 tile](../../outputs/validation/2026-09-26-v14-upload-icon/actual-upload-icon-native-70x70.png)。最终图标在固定 RGB三通道均<80 的阈值下与参考同 bbox `(30,254)..(49,274)`，暗像素 **125 vs 127**，但逐像素暗区 IoU 仅 **0.370**。当前边缘是纯黑矢量锐边，参考是灰阶抗锯齿；箭头几何仍有差异。**仅位置和面积接近，绝非1:1**。
- 下一阶段优先处理仍缺失的侧栏图标、列表文字重叠与酒红色占位轮廓。背景 RWI键盘 `Escape`/`cmd+a` 回执无可见效果需单独分析；本轮已经实测可用的 Paint 原生 `End Editing` 与 RWI双击数值文本框可作为当前工作流，不据此宣称所有后台键盘操作可靠。

没有 Git 提交或推送。原生图层与相邻阶段快照保留；Paint Undo 操作历史是否跨重启持久化仍未知。
