# macOS 坐标模型与验证规范

本文是所有 macOS 定位/点击/绘制换算的**强制规范**。任何硬编码的窗口原点、画布原点、
缩放比例都视为缺陷（2026-09-15 复刻事故的根因）。

## 1. 权威约定（Apple）

| 空间 | 原点 | y 方向 | 单位 | 谁在用 |
|---|---|---|---|---|
| CG 全局显示空间 | **主显示器左上角** | 向下 | 点(pt) | `kCGWindowBounds`、AX `kAXPositionAttribute`、`CGEvent.location` |
| Cocoa/AppKit 屏幕空间 | **主屏左下角** | 向上 | 点(pt) | `NSScreen.frame`、`NSWindow.setFrameOrigin` |
| 窗口图像 | 窗口左上角 | 向下 | **像素** | `computer.observe` 的 PNG、`pixel` 定位器、`computer.image` crop |

- 翻转公式（点子）：`cocoa_y = primary_height_pt - cg_y`；
  矩形：`cocoa_y = primary_height_pt - (cg_y + height)`。
- **点 ≠ 像素**：Retina/HiDPI 缩放模式下 `CGDisplayPixelsWide / CGDisplayBounds.width`
  **不是** backing scale（实测内建屏报 1.0，实际 2.0）。正确来源：
  `NSScreen.backingScaleFactor` / `NSWindow.backingScaleFactor`，或**直接测量
  "窗口图像 px ÷ 窗口 pt"**（自洽，无需假设）。
- 独立探针：`cargo run -p rcu-macos --example coords`（只读，不输入/不截屏）。
  实测 2026-09-15：main id=3 (0,0,2048×1152) 1x；id=1 (282,1152,1800×1169) 报 1x、
  实际 backing 2x；cursor 以全局 pt 返回。

## 2. 换算规范（运行时推导，禁止常量）

1. **控件/工具/按钮点击**：直接用场景元素的 `point`（已经是全局桌面 pt）。
   不要从截图量图标再换算——量到的是图标对的中点，系统性偏移（事故根因）。
   元素中心即 `point`；批量点按前用一次 observe 取全部目标点。
2. **窗口内像素 → 全局 pt**：
   `scale = image_size_px / window_size_pt`（同一次 observe 的两个值，逐轴计算）；
   `global_pt = window_origin_pt + image_px / scale`。
   `window_origin_pt` 取 `computer.discover` 的 `bounds[0..2]`（CG 点）。
3. **应用内部几何（如画布原点）**：两点探针实测拟合
   （画两条已知偏移的短线 → 回读图像位置 → 解线性映射），每次会话/窗口移动后重测。
4. **窗口截图裁剪**：crop 用**原始图像像素**，且必须完全落在 `image_size` 内。

## 3. 验证流程（iShot 交叉核对）

- 用 `computer.pointer_feedback {visible:true, point:[x,y]}` 把 A1 标记放在**计算出的全局
  pt**上；用户在 iShot（微信截图）读该标记的真实坐标 → 两者必须一致（±1pt）。
- 每类换算各验一次：屏幕尺寸/原点、窗口原点、窗口内像素、控件元素点。
- 验证失败时先核对本文第 1 节的"谁在用哪个空间"，再看第 2 节是否用了运行时值。

## 4. 实测记录（2026-09-15 全屏配置，Paint S 全屏于内建屏）

- 显示：main id=3 (0,0,2048×1152) 1x；内建 id=1 (282,1152,1800×1169)，
  CGDisplayPixelsWide 报 1x，**实际 backing 2x**（窗口图 3600px ÷ 窗口 1800pt = 2）。
- Paint S 全屏窗口：`bounds [282,1243,1800,1078] pt`，observe 图像 `3600×2156 px`，scale=2 ✓。
- 画布（文档）实测：**2144×728 px（与 Canvas Width/Height 一致，1:1 显示）**；
  在 `screencapture -l` 图中位于 (894,836)；`-l` 图比 observe 图高 **104 px**
  （差值来源未定论：阴影或标题带 🚧 待 iShot 复核），故画布在 observe 图中的原点
  ∈ {(894,732), (894,836)}，**必须由外部真值确定**。
- 换算（画布坐标 → 全局 pt，待原点确认后固定）：
  `global_pt = window_origin + (canvas_origin_px + canvas_px) / 2`，
  其中 window_origin=(282,1243) ✓。
- 已确证的换算（无需原点）：`scale = image_size_px / window_size_pt`（逐轴，运行时求）
  → global_pt = window_origin + image_px / scale ✓（例：图像 px(1200,700) → (882,1593)）。

## 5. 产品缺口（本轮发现）

- JS 车道（`rcu_computer_js`）未放开 `computer.pointer_feedback`（返回 `js_unknown_capability`）
  → 应把 #3a/#4/#5 新增工具加入 JS 允许清单。
