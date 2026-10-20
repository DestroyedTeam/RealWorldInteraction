# Paint V14 下一个侧栏图标：未保存的试笔与后台输入待核对（2026-09-26）

**后续状态**：Codex 重启后已用 RWI `Edit → End Editing` 确认网格是编辑覆盖层，错误圆形/过粗试笔已逐项 Undo；最终以较细描边绘制并保存独立155对象阶段。见[完成与逐像素核对](2026-09-26-v14-upload-icon.md)。下文是此前待处理时的原始阶段事实。

本记录是**未完成/未落盘的 GUI 试笔**，不是新增阶段成果。正式可编辑源仍为[149 对象九宫格阶段](../../outputs/paint-v14-source/paint-v14-20260925-sidebar-grid-nine-dots.paint)，484566 bytes、SHA‑256 `8fc26d801ca6940a723e6693406d136ceeb5959a9f2c3cea53b9e8e76f3d3943`；本轮结束前重新核对工作源字节未变。不要把当前应用内未保存内容或工具回执当成源文件已更新。

1. 参考第 4 个侧栏图标是深色圆角方框与上箭头，不是空白；原像素 420×680 参考局部暗色 bbox 约 `(30,254)..(49,274)`。RWI 在 BetterDisplay 虚拟屏 Paint 主窗选择 Round Rectangle，将 Border 打开、Fill 关闭。描边原为蓝色，用新鲜色板 AX `Input Color Manually` 设置 `#000000`。该按钮 AXPress 回 `-25204/uncertain`，查询原操作且**未重放**；独立发现新 `Input Color` 面板先落内建屏，再精确绑定并 `place_window` 到虚拟屏。HEX 文本与色井均回读黑色，确认后色板通过其自身 close AXButton 关闭，主窗描边色回读 `rgb 0 0 0 1`。
2. RWI 拖出一枚 Round Rectangle，回执 `dispatch=returned`。当前原像素[420×680 侧栏截图](../../outputs/validation/2026-09-26-upload-icon-trial/unsaved-after-roundrect-attempt-420x680.png)（SHA‑256 `b966b8cefef8316dd4a2f964acea02904692d977dc28efc1b45a01e9784f3b58`）显示该位置出现**远大于参考的方框及内部网格/控制柄样线**。不能确定它是实际图形、选中变形覆盖层还是二者叠加；不能保存为目标图标。
3. 首次 `Escape` 被 RWI 判为 `user_active`、输入前拒绝，未派发；原请求没有重放。其后两次新的 `Escape` 和一次空白处单击回执虽为 `returned`，独立截图仍未证明覆盖层消失或目标达到。一次色板像素关闭尝试被 `stale_reference` 拒绝并围栏；查询原请求、原生键鼠/写入者空闲后显式恢复输入域，旧操作不可重放，随后改用色板 AX close 成功。用户会话持续有硬件键鼠活动，避免再以回执推断应用效果。

**接续要求**：在用户空出 Paint S 虚拟屏后，重新 `targets→discover→bind`，比较未保存试笔和上方原像素证据；优先查明/清理这一尝试（必要时确认最末 Undo 的作用域，不能盲目撤销用户可能的新操作），重新观察是否真为选择覆盖层。只有实际图形、独立图层及尺寸核验通过后才 `cmd+s` 并新建阶段快照。若无法安全判定，不覆盖149对象的正式源。另需单独调查背景 `Escape`/blank click 回执无可见效果，不因一次经验放宽身份或输入门禁。
