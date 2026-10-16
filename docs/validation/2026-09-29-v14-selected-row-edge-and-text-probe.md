# Paint V14 选中行边缘与原生 Text 可行性试验（2026-09-29）

承接[185对象酒红环遮盖阶段](2026-09-28-v14-selected-row-ring-mask.md)。工作源起始 SHA‑256 `85f1f52473a4e307822784c3e6fa336af3287a8d026fbdfac308e8304648a52c`。RWI重新选择 macOS `mac-host`，发现/绑定 Paint S PID14404、`cgwindow:2189`，确认窗口仍在 BetterDisplay UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`；四项 TCC 均有效。用户的 GameHub/游戏留在主屏，未对其输入。

## 顺序与验证

1. **文字重建试验未留在画稿。**参考第5行标题暗像素 bbox `(146,425)..(242,439)`，旧画稿为 `(142,426)..(237,440)`，说明烙在底图里的标题约左偏4px、下偏1px。RWI真实 Text 工具在选中行空白处创建 AXTextArea，`computer.type_text("测试")` 返回派发且 AX 回读 `value="测试"`；Escape 结束编辑、一次 Undo 后恢复 Rectangle 工具。对保存的185对象截图，全幅420×680没有任何RGB通道差异超过2，工作源 SHA‑256仍为起始值；试验文字没有混入最终源。
2. **正式文字暂不硬改。**Text 工具确实提供 `.AppleSystemUIFont`、Regular、字号18和文字颜色井；但字号 AXComboBox 的 `computer.type_text("16")` 两次均在派发前报 `text_target_not_focused`，单键 `1` 在解析期报 `unsupported_key`。文字色井弹出的 CG色板虽可精确发现 `cgwindow:12733`，绑定后 `computer.snapshot` 只有 `visual_only=true`、无AX控件与输入引用；未凭截图坐标给只读窗派发输入。一次文字色井点击返回 `local_pixel_surface_changed/uncertain`、输入域围栏；已查询原请求、没有重放，独立原生 HID 显示0按键/0鼠标键及writer idle后用 `computer.recover_input` 解围。`Edit`菜单 CG窗口一度跨过显示器上边缘（`[2509,-7,423,754]`、display null），精确绑定后的观察报 `visual_display_unknown`，没有用另一屏或近似窗口顶替。形状填色板 `cgwindow:8474` 则仍可正常AX观察；这是**Text控件/弹窗路径的现场差异**，不是整个Paint或RWI失效的证明。后续需修复并重新验收字体/取色/菜单路径，再用独立 DrawText 重建烙底列表字。
3. **安全的边缘修正。**参考与旧画稿的 x72–141 平坦区逐行对照显示：y407–408 参考 `#F2F2F2`，旧底图为约 `#D9D9DA`；y480–494 同理，且不含头像/文字。RWI使用 Rectangle、Fill `#F2F2F2`、Border off，在这两段分别画70×2和70×15的独立矩形。输入颜色对话框默认落主屏，RWI精确发现、绑定并立即 `place_window` 到虚拟屏，读回位置确认；色板 `Input Color Manually` 的 AXPress 为 `-25204/uncertain`，只查询原请求并依据独立发现的实际对话框继续，未重放。
4. **原像素与落盘。**[改前185对象原生截图](../../outputs/validation/2026-09-29-v14-selected-row-edge/01-before-185-objects.png) → [保存后187对象原生截图](../../outputs/validation/2026-09-29-v14-selected-row-edge/02-final-saved-187-objects.png)；[参考／改前／改后3倍并排](../../outputs/validation/2026-09-29-v14-selected-row-edge/reference-before-after-3x.png)。两张正式截图均是 RWI `computer.image` 从原始 `2868×1320` 单窗 artifact 裁 `[1000,390,420,680]` 的原像素，而非预览放大；3倍图仅供肉眼比对。按RGB单通道差异>3计，变化**恰好1190像素**（70×2＋70×15），bbox `(72,407)..(141,494)`；头像 x78–126/y422–470和右侧文字 x142–350/y420–465均0改变。顶部70×2区域RGB MSE **631.68→0.46**，底部70×15区域 **609.42→0.37**。

正式工作源与[可编辑阶段 `.paint`](../../outputs/paint-v14-source/paint-v14-20260929-selected-row-edge.paint)字节相同：491377 bytes、SHA‑256 `1066a6a78d7c69716ff199e67776a0f47cf2e6439391b316af1b7ee04088cc57`；双层NSKeyedArchiver核对**187对象**（12 Image/41 Rectangle/99 Line/23 Ellipse/9 RoundRectangle/3 Text）。与上阶段相比仅新增2个独立Rectangle，试验Text未保留。[manifest](../../outputs/paint-v14-source/paint-v14-20260929-selected-row-edge.manifest.json)。

仍有头像位置、选中行左侧灰底整体形状、列表文字偏移/重影，以及其他行相同深灰头像列等结构性差异。此轮没有将试验文字当作完成，也没有宣称整图1:1或字体/色板已可用。无Rust产品代码改动、部署、Git提交或推送；不冒称重跑cargo门禁。`rtk git diff --check` 单独核对。
