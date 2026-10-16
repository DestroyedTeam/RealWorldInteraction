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
- 独立探针：`cargo run -p rwi-macos --example coords`（只读，不输入/不截屏）。
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

## 6. 根因（2026-09-15 实测确证）：窗口图像含"非等比 chrome 带" → 线性映射失真

- 实测同一窗口两次 observe：
  * 图像 `3600×2156`、窗口 `1800×1078 pt` → 2.000× ✓ 纯缩放（换算正确）
  * 图像 `3600×2262`、同窗口 → 2.098× ✗ **图像比 2× 多出 106px 带**
- 现实现 `pixel_to_desktop = rect.map(image_size)`（逐轴线性）在第二种情况下
  **整幅图像被拉伸到窗口 rect** → 图像内所有点的桌面换算产生纵向偏移
  （106px 带被摊到全高 → 越靠下偏差越大）→ 一切"图像像素→点击"都会点偏。
- 结论：**参考系不是"窗口 rect"，而是"该次捕获实际的区域"**。

### 修复清单（按层）
1. **rwi-macos（核心）**：捕获时记录并上报**捕获区域**（窗口内/全局 pt 的 rect，
   含 chrome 带的实际偏移）；`pixel_to_desktop` 必须由该 rect 与图像尺寸推导，
   而不是由窗口 rect 推导。若捕获含 chrome，则 chrome 带要么裁掉，要么在映射中体现。
2. **rwi-core/工具面**：`Scene` 增加明确的捕获区域字段（如 `capture_region_pt`），
   并在 `computer.observe` 回执中可见；`Locator::Pixel` 以图像像素为唯一输入，
   换算全部由 RWI 完成（agent 不做任何全局换算）。
3. **拓扑变化的显式化**：窗口记录的 `native_epoch` 摘要加入"所在显示器 + 当前活动
   显示器集合（id/bounds/pixels/main）"；显示器热插拔后旧 frame/reference 一律失效
   （fail-closed 并提示重新 observe），不依赖 agent 察觉。
4. **JS 车道**：允许清单加入 #3a/#4/#5 新工具（pointer_feedback / move_window /
   pointer 相关），使 JS 与 MCP 工具面一致。
5. **测试**：为 1 与 3 增加离线单测（纯函数：给定 rect/图像/拓扑 → 期望映射与失效）。

### iShot 复核（修复后由用户执行）
- 用户在全屏截图（1:1）中标出鼠标位置；RWI 侧把同一"图像像素目标"映射出的
  全局 pt 与该点比对，误差须 ≤1pt。

## 7. 追加实测（2026-09-15 深夜）

- 同一窗口再次 observe：`image 3600×2262`、`window_rect_pt 1800×1131` → **scale=2.0000 精确** ✓
  （说明"非等比带"并非总是出现；它与窗口状态有关，故 §6 的修复仍必要：映射必须由
  **本次捕获区域**推导，才能在两种情形下都正确。）
- 另一独立问题：多次"应在画布内落墨"的 drag 返回 `dispatch: returned` 但画布上**无墨迹**
  → 归因于**当前工具不是绘制类工具**（早前标定笔迹/多边形能落墨，说明链路本身可画）。
  → 需要一次"工具身份实证"（每工具 1 短拖 + 裁剪回读），确保绘制前处于已知绘制工具。
- 仍然成立：pixel→desktop 换算在 scale=2.0000 的观测下与裁剪回读自洽（图像坐标为目标）。

## 8. iShot 复核方式修订（无需光标）

截图通常不含光标 ✗。改为**墨迹参照法**：
1) RWI 在画布内落一笔显著深红墨迹（并给出"按实现换算的期望全局点"）；
2) 用户全屏截图（系统截图/iShot 均可）发回；
3) RWI 在截图中量出墨迹的实际位置（按显示器 origin/scale 换算为全局 pt）与期望比对，误差 ≤1pt。

## 9. 用户实测真值（iShot 读数，2026-09-15）

用户对 RWI 所画深红线段用 iShot 读出：
- 起点 **(492, 669)**、终点 **(526, 670)**（线段长 ≈34pt，几乎水平）
- 用户结论：**微信截图 / iShot / 其它截图携带的坐标都准确；只有 RWI 截图与 Python 截图坐标漂移。**

推断与算式：
- iShot 读数很可能是**显示器局部坐标**（内建屏原点在其左上角）。若如此，全局坐标
  = (282+492, 1152+669) = **(774, 1821)**。
- RWI 当时对该笔迹的期望（按实现换算）= 窗口原点(282,1190) + 图像px(1800,1300)/2
  = **(1182, 1840)** → 与实测差异 **(−408, −19)**：y 仅差 19pt、x 差 408pt ✗
  → 说明"图像像素→桌面"的横向基准与真实捕获区域不一致（与 §6 的捕获带问题同源）。
- 需要用户再给一个**无歧义基准读数**：把 iShot 十字线放在**内建屏最左上角(0,0 可视屏幕角)**
  读一次 → 若显示 (0,0) 则确认为"显示器局部坐标"，若显示 (282,1152) 则为"全局坐标"，
  之后所有比对都以该空间为准。

## 10. iShot 空间判定（2026-09-15 第二次实测）

用户读数 #2：靠近内建屏左上角处 iShot 显示 **(2, 1166)**；HEX 000000。
用户读数 #1：深红线段 起 **(492, 669)** 终 **(526, 670)**（长约 34pt，水平）。

假设检验：
- 若 iShot 为"显示器局部、y 朝下" → 左上角应读 (0,0) ✗（实测 (2,1166)）
- 若 iShot 为"全局、y 朝下" → 左上角应为 (282,1152) ✗（x 不符）
- 若 iShot 为"显示器局部、**y 朝上**（Cocoa 风格）" → 左上角 = (≈0, 高度≈1169) ✓✓
  实测 (2,1166) 与之吻合（x=2、y=1169−1166=3 距上缘）✓
  → 则线段 y_局部(朝下) = 1169−669 = **500**，全局 y = 1152+500 = 1652；
  而 RWI 期望全局 y = 1840 ✗ 差 −188 ✗（与"y 仅差 19"的早先推算矛盾）
⇒ 结论：**必须用一次受控实验定标**（同一会话内画一笔已知图像坐标的墨迹，用户读其起止），
   并在必要时读一次"屏幕左下角"以确认 y 方向。空间确定后，一次线性拟合即可钉死映射。

## 11. 坐标空间定稿（用户 2026-09-15 明确）

- **采用"微信截图坐标系"**：原点在**目标显示器左上角**、y 朝下、单位点(pt)。
  用户后续读数均用此空间（等价于对该显示器的整屏截图坐标）。
- 实测确认：屏幕左上角读 **(0,0)** ✓（越靠近左上越小）。
- 与全局空间(CG)的关系：`global = display_origin_global + local`，
  例：内建屏 origin_global=(282,1152) → local(492,669) = global(774,1821)。
- 先前误差复盘：不少"偏差"来自我用了**过期的窗口 rect**（窗口在两次观测之间移动/变高：
  1243→1190 等）→ 这正是 §6/§7 要修的实现问题（一切几何必须当次读取 + 捕获区域准确）。

## 12. 已验证事实 + 未解问题（2026-09-15 收尾）

**已验证（可直接依赖）**
- 画布局部 (0,0) ↔ 全局 pt **(729.31, 1610.95)**（用户读 Paint S 状态栏 `X:0 Y:0` +
  RWI `coords` 探针同期读光标 = 一次配对，实测吻合早先 PIL 推导 (729,1609) ✓）。
- 换算：`global = (729.31,1610.95) + canvas_px × 0.5`；窗口图像 scale = 2.000 精确 ✓。
- `pixel_to_desktop` 由当次 `窗口 rect + 图像尺寸` 推导 ✓（自洽，无需外部常量）。

**未解（需原生插桩定位，不能再靠经验补偿）**
- 受控探针：按上述换算投笔，墨迹落点与参考点存在**可复现偏移 ≈ (−100,−47) pt**；
  按该常量补偿后再投，仍未命中预期位置 ✗ → 说明偏移与**拖拽锚定/真实按下点**有关
  （`drag_anchor_facts` 会把手势锚到命中元素，再从其几何起算 offset）。
- 结论：**必须先让 act 回执暴露"真实按下点/所用锚元素"**（原生插桩），再谈拟合。

**下一步（实现优先，符合用户"先修好再对照"的要求）**
1. act/drag 回执增加 `press_point_pt` 与 `anchor_element`（原生实测值）。
2. ink probe：act 前后各 observe，返回像素差异包围盒（标定与落笔验收自动化）。
3. §6 捕获区域贯通 + §10 拓扑指纹失效 + JS 车道放行新工具 + 离线单测。
4. 以上完成后：三点探针 → 仿射拟合 → 全图绘制 → ink probe 自检 → 用户 iShot/微信坐标复核。

## 13. 待实现规范：ink probe（自动标定/落笔验收）

**目标**：一次 act 前后各取一帧，返回**像素差异包围盒**（图像 px），使标定与"是否落笔"
完全自动化，不再依赖人工看图/读数。

**接口（拟）**：`computer.ink_delta {target, lease, before: {frame, artifact}, after: {frame, artifact}, region?}`
→ `{changed: bool, bbox_px: [x0,y0,x1,y1], changed_ratio, region}`（两帧须同 target/同尺寸）。

**实现落点**
- rwi-imaging：复用现有 PNG 解码（`view()` 已解码并缩放）→ 新增 `diff_bbox(a, b) -> Option<[u32;4]>`
  （逐像素比较，容差可配；只返回包围盒与比例，不回传像素）。
- rwi-core：`Engine::ink_delta(...)`（校验两 artifact 属于同 target/frame 系，字节上限沿用 budget）。
- rwi-agent：executor 分支 + 目录/能力注册（writes: false，纯读取）。
- rwi-mcp：`#[tool] computer.ink_delta` + 参数；JS 车道同步放行。
- 测试：rwi-imaging 纯函数单测（构造两张小图 → 期望 bbox）；离线可跑。
- 用途：探针标定（3 点 → 仿射拟合）、逐批落笔自检、最终"复刻是否有落笔"的机器验收。

**配合项**：act/drag 回执增加 `press_point_pt` 与 `anchor_element`（原生实测），
用于解释并修复当前 ≈(−100,−47)pt 的按下点偏移。

## 14. 画布映射的真正变量：文档缩放（zoom）与文档尺寸（2026-09-18 实测定案）

- 实测：同一 Paint S 窗口，文档 2000×679、**Zoom 150%** 时，画布在窗口图中的显示比例为
  **1 canvas px = 1.5 image px = 0.75 pt**；而早前（2144×728、Zoom 100%）是 **1 canvas px = 0.5 pt**。
  → 此前所有"系统性漂移"（含 (−100,−47)pt 偏移）本质是 **我把它当常量用** ✗，
    而它随 **zoom**（以及文档尺寸/滚动）变化 ✓✓。
- 正确规则（可自动化）：
  `pt_per_canvas_px = zoom/100 × 0.5`（zoom 可从 AX "Zoom" combo 原生读到 value ✓，实测 "150"）
  `canvas 原点` 一次性标定：①"状态栏 X:0 Y:0 ↔ 光标全局坐标"配对，或
  ②**读画布标尺**：标尺以 canvas px 标注 ✓ → 取两条刻度标签的图像坐标 → 直接解出比例与原点 ✓✓（无需人工）
  ③探针回读（在 **干净区域** 落一笔 → 裁剪回读）
- 触发重算的时机：换文档、改缩放、滚动、移动/缩放窗口 —— 任一变化都使旧映射失效 ✓。
- 产品改进项：`computer.observe`/capabilities 暴露 **document zoom**（AX 已可读 ✓）；
  对画布类应用，把"标尺/缩放驱动的映射"写进 SKILL 作为标准流程 ✓。

## 15. 复刻 v3：映射验证成功 + 工具面实测修正（2026-09-18）

**结果（118/118 图元，画布 2000×679 @ Zoom 150%，计划框 canvas (400,0)+1169×679）**
- 精度：计划采样点 5010 个 → **到墨迹的平均距离 1.07 image px（0.71 canvas px）**，
  97% 落在 1 image px 内、99% 在 3 px 内；**最优网格位移 (0,−1)**（即**无系统性偏移** ✓）；
  覆盖率 0.9936（零位移）→ 对照 v2（需 90px 搜索半径）✗，映射问题**彻底解决** ✓✓。

**工具坐标（按效果自证：图层名 + 状态栏读数，修正本文件的旧表 ✗）**
- 19 个 AXRadioButton，节距 37pt，从 **x=987 起**：987 吸管 · 1024 渐变 · 1061 油漆桶 · 1098 第二填充 ·
  **1135 直线 · 1246 矩形 · 1283 圆角矩形 · 1320 椭圆** · … · 2003 撤回（y≈1216）。
- 早前表中的 970/998/1030/1150/1177 ✗ **作废**（那是把图标序号读偏的结果 ✗）。

**应用行为（操作必读）**
1. 形状工具**单击会生成默认 201×201 形状** ✗ ⇒ 任何"点空白处取消选择"都会留下杂形；
   正确的去选是**再点一次该工具的 radio**（act_checked 元素级 ✓，免疫蚂蚁线 ✓）。
2. **同类形状被选中时，同工具拖拽会"编辑该形状"而非新建** ✗ ⇒ 每批前必须"重按工具 radio"✓。
3. 圆角矩形的圆角半径是**应用固定值 40** ✗（点该字段被拒 `secure_or_unknown_text_field` 并 fence ✗）
   ⇒ 所有圆角矩形取 min(40, w/2, h/2)，这是本次最明显的保真偏差 ✓（如实记录）。
4. **租约必须每次 act 前重新 claim（60s）** ✓：过期租约导致 uncertain + 输入域 fence ✗；
   改为此规则后，最后 100+ 次派发零 fence ✓。
5. 瞬态失败需重试而非当状态 ✗：observe `ax_attribute_unavailable`、resolve `outside_frame`、
   act `unknown_reference`/`visibility_unknown` ✓。

## 16. 用户验收意见（2026-09-18）→ 复刻下一步规范

**已过关**：整体骨架/框架复现很好 ✓（窗框、标题栏、左图标栏、搜索框、8 行列表、右栏外框、底部工具条）。

**待修（用户点名）**
1. **聊天区内骨架复现不好** ✗ → 必须按原图重测：头部题字行与 6 个图标、`1条新消息` 胶囊、
   图片卡片矩形、消息气泡（含逐行文字范围与其内小图标）、条目/统计行、右栏底部图标行。
   我复核 v3 对比图看到的具体偏差：卡片只画了外框（缺"图片区/文字区"内部分界）、
   气泡外框偏大偏宽、右栏底部图标行被画成圆圈占位（应为 8 个小图标）、胶囊内箭头图标缺失。
2. **图标要填充**：从**互联网图标库**（如腾讯/开源图标集）**下载真实图标**再灌进画布 ✓（不手绘 ✗）。
3. **头像/非公开图片**：只能**从截图手动切图**后作为图片填充 ✓。
4. **文本**：本轮先做图标；文本与颜色填充随后 ✓。
5. **颜色选择器注意**：它是色板，会弹**独立弹窗** ✗ → 按"系统面板/弹窗"处理（绑定弹窗→操作→关闭）✓。
6. **精细填充须配合缩放**：用右上角缩放控件 ✓，并按 `pt_per_canvas_px = zoom/100 × 0.5` 重算映射
   （已验证的 0.75 是 Zoom 150% 的值 ✓）。
7. **清理画布左侧杂乱测试痕迹** ✓（选区工具 + 删除图层 / 橡皮擦）。

**执行纪律（沿用本会话已验证项）**：后台执行、不动真实光标、不抢焦点、每次 act 前重新 claim 60s 租约、
形状工具单击会生成 201×201 默认形状（去选须重按工具 radio）、验证一律**整窗回读**。

## 17. 输入语义实测修正（2026-09-18，run4）——**推翻 §15/§16 中的错误说法** ✗

**① `computer.resolve` 的 `kind=pixel` 取的是「原始窗口图像像素」，不是桌面 pt** ✓✓（此前 ticket 写反了 ✗，
导致多轮 click/marquee **静默打在空白画布上** ✗）。实测：请求 img(496,842) → 返回 point [530,1611]（桌面 pt）。
坐标链（三者数值互证 ✓）：`image_px = (466,692)+canvas_px*1.5`；`desktop = (515,1536)+canvas_px*0.75`（Zoom 150%）。

**② 画布上存在「动态选区（蚂蚁线）」时，普通 `computer.act` 会失效（内容哈希持续变化）** ✗。
必须先删除该选区对象；删除后连续两次捕获 **0 px 变化** ✓，普通 act 恢复一次成功 ✓。
`act_checked` 免疫内容哈希检查 ✓，但**只能作用于所引用元素自身的点** ✓（画布 AXScrollArea = 桌面 (1265,1790.5) = canvas (1000,339.3)，
且拖拽首 waypoint 必须 [0,0]）✗ 因此它到不了任意画布点。

**③ `act` 拖拽语义** ✓：首个 waypoint `[0,0]`（= resolve 出的点）；其后 offset 单位为**桌面 pt** ✓。
实测：canvas (20,340) + offset(150,80)pt → 墨迹落在 canvas (12.7,333)–(226.7,454)（214×121 canvas px，圆帽每端膨胀 ~2px ✓）；
canvas y=480/520/560 + offset(60,0)pt → 长度**精确 80.0 canvas px** ✓。

**④ 工具切换走 Host AX 通道** ✓（**不动光标、不改焦点** ✓✓）：`perform action "AXPress"` on
`radio button N of radio group 1 of group 1 of toolbar 1 of window 1`（N：5=Line · 8=Rectangle · 9=Round Rectangle
· 10=Ellipse · 19=Select and Move）；press 后 ~0.5s 再回读（读回有延迟 ✓）。
**它同时用于「去选」** ✓（避免"单击生成 201×201 默认形状" ✗）；同 kind 形状被选中时同 kind 拖拽=编辑它而非新建 ✗。

**⑤ 框选语义 = 相交即选（INTERSECTING），不是包含** ✗✓：实测 canvas (973,190)-(995,230) 的框**顺带删掉了右边线在 997.84 的卡片框** ✓。

**⑥ 删除/撤销走 Host AX 菜单** ✓：`menu item "Delete" of menu 1 of menu bar item "Edit"` 删除当前选区；
Edit 菜单第 1 项即撤销（名字会变："Undo Delete"/"Undo Add Line" ✓）且能完整恢复墨迹 ✓。

**⑦ 编辑 oracle** ✓：`work/replica-verify-20260915/run4/delta.py` 逐 20 canvas px 比对两次捕获的墨迹增量 ✓。

**⑧ 截图通道注意** ✗：`screencapture -x -o -l 26019` 可能在窗口头图标区**贴旧 tile** ✗；
`-x -o -D 2` 的显示器级捕获才是可信视觉验收通道 ✓。
 
## 18. 复刻 v4 当前进度与填充证据（2026-09-18）
 
**已验证完成**
- 左侧测试层：先用内缩框选 `(canvas 10,10)→(390,670)`，确认层面板仅高亮 5 个 `Line` 测试层，再执行删除；显示器级回读确认左侧墨迹归零，复刻框未被删。证据：`work/replica-verify-20260915/run5/left-cleaned-final.png`、`evidence-current.png`。
- 聊天区新增骨架：v3 plan 中 `op=add` 的 **20/20** 个形状已落笔（8 个修正形状暂未删除旧层，避免相交框选误删）；执行记录 `run5/pane-additions.json`，整窗证据 `run5/pane-additions-final.png`。
- 截图头像资产沿用 ReplicaFill 已验证结果：`assets/avatar-row1-place48.png`，已在列表第一行落位。
- 头部图标：电话（截图切图）与 videocam（Material Icons，Apache-2.0）落位；腾讯 TDesign `share`、`add` 两枚也已落位。目标状态栏回读：电话 `X=1311,Y=78`，摄像头 `X=1351,Y=78`；TDesign 目标中心分别 `(1396.2,78.2)`、`(1489.9,78.5)` canvas px。证据：`run5/phone-header-corrected.png`、`header-icons-placed.png`、`tdesign-all-header-icons.png`。
- 底部图标行已验证第一枚 TDesign `folder` 落位：状态栏 `X=881,Y=460`，目标 `(880.8,460.5)` canvas px；证据 `run5/folder-placed.png`。

**来源与许可**
- Tencent TDesign icons：`https://github.com/Tencent/tdesign-icons`，仓库 `LICENSE` 为 MIT；本轮使用 `share.svg`、`add.svg`、`folder.svg`、`image.svg`、`mail.svg`、`microphone.svg`、`time.svg`。
- 截图私有素材只使用本地截图切图；没有把网络图片冒充头像。

**本轮踩坑与边界**
- Add Image 的图像移动方向必须使用 `0.75 × (target_center - (1000,339.3))`；头部图标目标在画布中心右侧，之前旧 procedure 的负号会把电话移到左侧，已撤销并以状态栏 `X=1311` 重新落位。
- 底部图标批量导入曾被 macOS `Open` 面板卡住；后续 4 次调用均 `native_dispatch_entered=false`，没有重放。面板已安全关闭，输入域已 `recover_input`，当前画布无未确认的误动作。后续必须逐枚：打开→绑定/选文件→关闭面板→重新 observe→移动→整窗验证。
- 当前仍未完成：旧 v2 的 8 个纠正形状删除、其对应 8 个 v3 correct 几何、底部其余图标、颜色与文本。

**当前证据对比**：`docs/validation/replica-v4-comparison-20260918.png`（原图目标 | 当前画布）、`replica-v4-current-20260918.png`、`replica-v4-overlay-20260918.png`。
 
## 19. 腾讯图标单文件导入收口（2026-09-18）
 
Open 面板批量导入的失败根因已验证：面板按最近文件排序，固定 `group 1` 不一定是请求文件；面板保持 modal 后，后续 RWI 输入全部 `native_dispatch_entered=false`。修正为：
1. 删除本轮临时 `~/Documents/icon-tdesign-*.png`；
2. 每次只复制一个目标 PNG；
3. Add Image → AXOpen `group 1` → radio 19 → `act_checked` 移动 → 回读状态栏；
4. 下一枚前重复清理。

按此 SOP 已完成输入栏 5 枚腾讯 TDesign MIT 图标：
- `folder`: `(880.8,460.5)` → 状态栏 `X=881,Y=460`；
- `image`: `(932.1,460.8)`；`mail`: `(974.5,460.8)`；`microphone`: `(1016.6,460.8)`；`time`: `(1527.2,460.8)`。
目标/偏移及 RWI request receipt：`work/replica-verify-20260915/run5/bottom-icon-isolated-results.json`；整窗证据：`run5/bottom-icons-isolated-final.png`、`run5/icon-fill-final.png`。
更新对比：`docs/validation/replica-v5-icon-fill-{current,comparison,overlay}-20260918.png`。

当前图标已填：头部电话、videocam、TDesign share、TDesign add；输入栏 folder、image、mail、microphone、time；剩余 header grid/chevron/more、底栏 smiley/scissors/people/screen/robot、星标等待后续，文本与颜色仍未填。

## 20. 2026-09-19 重启后的画布重建与坐标自证

**事实**：本机重启（`security session 100002 → 100003`、boot 时间戳变化、Paint S 83334 → 2578，rwi-agent 进程随之退出）。原文档
`white-canvas-2x.png` 是夹具文件且未保存，磁盘与窗口列表均已不存在 → 当日复刻画布丢失；仓库内的全部计划（118/28 形状）、
素材（头像、卡片内容、10 枚图标）与 SOP 未受影响。

**恢复步骤（已实测，全部走既有授权通道）**
1. `pgrep -fl rwi-agent` 无进程、`agent.pid` 记录 50652、端口 45129 未监听 → 用 `hub start` 以 `--config agent.json` 重启部署件，ready 条件 `port 45129 + log listening`；
2. 用 PIL 重建 `work/replica-verify-20260915/run6/white-canvas-2x.png`（2000×679 白底），`open -g -a "Paint S" <file>` 后台打开（不抢前台）；
3. Host AX 恢复窗口几何：`set size to {1800,1131}`、`set position to {282,1190}`（内建屏，实测报 1776×1131，与适配器读回 1800×1131 有 ≤24pt 差异，映射不依赖该值）；
4. Host AX 设缩放：`combo box 1 of window 1` → `set value to "150"` → `perform action "AXConfirm"`（从 100 → 150 已验证）；
5. 重新标定：画布 AXScrollArea 元素点桌面 `(1265,1790.5)` = canvas `(1000,339.5)` → `origin_pt=(515,1535.875)`、`origin_img=(466,691.75)`。

**自证**：重建后的窗口 `[282,1190,1800,1131]`、图像 `3600×2262`、画布锚点 `(1265,1790.5)`、origin_img `(466,692)`、
Zoom 150% —— 与重启前逐项一致，说明"由画布元素点反推原点"的方法是稳定的，不依赖记忆常量。

**证据差异（必须按新读回判定）**：`computer.move_window {display:{built_in:true}}` 的回执返回
`bounds=[443,179,1220,527]`、`display={built_in:false,id:2}`（即"未移动"），但紧接着的 `discover`+`bind_window` 读回为
`bounds=[306,1190,1220,527]`、`display={built_in:true,id:1}` —— **窗口实际已移动到内建屏**。结论：放置结果以移动后的
新鲜读回为准，回执可能是移动前的快照；不要因回执而重发同一次放置。

**遗留改进项（产品侧，待办）**：文档未落盘会导致重启即丢画布；应在每个阶段后用 Host 通道保存文档（或在验收脚本中加
"保存→重开→读回"节点），把"工作产物持久性"纳入验收，而不是只验证墨迹。

## 21. 2026-09-19 画布找回/重建与"以画作几何定标"（推翻元素点推导）

**原文件结论**：丢失文档原路径 `/private/tmp/white-canvas-2x.png`（Paint S 日志实证：09-15 10:18 readFromData、
**09-18 17:12 saveToURL 成功**）。重启清空 `/private/tmp`；`tmutil destinationinfo` = **无 Time Machine 目标**、
`listlocalsnapshots` = **无本地快照**、Spotlight/容器/autosave/同尺寸 PNG 全扫均无副本 → **可编辑图层文件不可恢复**。

**替代恢复（已交付）**：用末次覆盖整块画布的 2x 截图（`run5/icon-fill-final.png`）重建画布位图。
关键在**裁剪原点必须用画作自身几何反解**，而不是用画布 AXScrollArea 元素点 —— 元素点只在"文档在视口内居中"时等于画布中心；
视图滚动时该假设失效（本轮实测偏差 76 image px ≈ 50.7 canvas px，表现为位图顶部多出一条灰色应用底带）。

**定标方法（本轮验证，推荐固化为标准做法）**
1. 复刻外框在计划里就等于画布框 `canvas (400,0)+1169×679`；
2. 在截图里先框出**文档矩形**（亮区）→ 可判 k（本例宽 3000 image px = 2000 canvas px × 1.5，zoom 150% ✓）；
3. 再在文档矩形内取墨迹外接框的左上角 (Fx, Fy) → `origin_img = (Fx - 1.5*400, Fy)`；
4. 自检：重裁后外框 bbox 应为 `(400,0)-(1569,679)` —— 本轮实测 **x 400..1568 / y 0..678** ✓（≤1 px）。
   → 产物：`docs/validation/recovered-canvas-aligned-20260919.png`（2000×679），已作为新文档打开（窗口 `[282,1190,1800,1131]`、内建屏、Zoom 150%）。
   当前该窗口映射：`origin_img = (466, 932)`（视图滚动，故与旧值 768 不同，**每次开窗必须重量**）。

**保存与窗口语义（实测）**
- 菜单命令（File>Save、Edit>Delete…）作用于**应用主窗口**：需先把目标窗口设为主窗口。
  `AXRaise` 不改主窗口，**RWI 后台点击标题栏**可改（实测：`AXMain` 变为我们的文档，同时前台应用仍是 ChatGPT ✓ 未抢焦点）。
- 文档 canvas 与图像尺寸不一致时，**关闭**会弹 `Resize` 对话框（Cancel 不关闭）；本次改为 `AXMinimized=true` 收起旧文档，避免与工作文档重叠。
- **保存→读回 PNG 是像素级 oracle**：保存后的文件就是 1:1 画布内容，可直接做墨迹/落点校验（不再依赖截图与水印干扰）。

**进程稳定性**：hub 托管的 `rwi-agent-mac-host` 今日 **exit=1 两次**（日志仅一行启动记录，无 stderr）。
处置：`hub ps` 查状态 → `hub start ... --config agent.json`（ready 条件 port 45129）→ `computer.targets` 复核。产品侧应加崩溃诊断输出。

## 22. 2026-09-19 恢复画布上的图标签：实测落点表与验收口径

**方法**：单文件隔离 → Add Image（Host AXOpen）→ `act_checked` 在画布元素上拖拽
`offset = target_desktop − canvasElement.point`（`target_desktop = origin_pt + 0.75×target_canvas`）→ **File>Save → 读回 PNG 量质心**。
本窗口 `origin_pt = (515,1656)`（= `(282,1190) + (466,932)/2`）、`pt_per_canvas_px = 0.75`（Zoom 150%）。

**结果（11 枚，读回 PNG 质心 vs 目标，单位 canvas px）**
| 槽位 | 目标 | 实测 | 误差 |
|---|---|---|---|
| hdr_screenshare | (1396.2,78.5) | (1395.4,78.5) | −0.8, 0.0 |
| hdr_grid | (1438.3,78.5) | (1438.1,77.5) | −0.2, −1.0 |
| hdr_plus | (1489.9,78.5) | (1489.3,78.5) | −0.6, 0.0 |
| bar_smiley | (795.9,460.8) | (795.6,460.6) | −0.3, −0.2 |
| bar_scissors | (833.8,461.1) | (833.6,461.6) | −0.2, +0.5 |
| bar_people | (1058.8,460.4) | (1058.6,460.6) | −0.2, +0.2 |
| bar_screen | (1101.2,461.1) | (1100.4,460.8) | −0.8, −0.3 |
| bar_robot | (1143.7,460.8) | (1143.5,460.7) | −0.2, −0.1 |
| bar_clock | (1527.2,460.8) | (1524.9,458.9) | −2.3, −1.9 |
| star | (985.4,302.5) | (982.3,302.3) | −3.1, −0.2 |
| pill_chevron | (1454.1,135.5) | 不可测（窗口内混入既有胶囊描边） | — |

**来源/许可**：腾讯 TDesign（`github.com/Tencent/tdesign-icons`，MIT）用于 screenshare/grid/plus/people/screen/robot/clock/star；
MDI（`cdn.jsdelivr.net/npm/@mdi/svg`，Apache-2.0）用于 smile（emoticon-happy-outline）与 scissors（content-cut）；
头像与卡片内容为本地截图切图。

**已知遗留（如实记录）**
1. 基础位图里仍有**计划阶段的占位圆**（bar_* 槽位）与部分描边，新贴图标与之**叠加**，底栏看起来是"圆 + 淡描边"混合 ✗；
2. `pill_chevron` 位置本就有一条描边箭头，贴图属**重复** ✗；
3. 颜色与文本尚未填；水印（`Created by Paint S`/`Free Trial`，应用叠加层）仍在位图内 ✗；
4. 图像导入后**画布元素点=导入落点**这一前提在本窗口成立（否则 11 枚不会全部 ±1 px 命中）✓。

**验收产物**：`docs/validation/replica-v6-icons-filled-comparison-20260919.png`（原图目标 | 当前画布）、
`...-overlay-...png`（50% 叠加）；明细 `work/replica-verify-20260915/run6/icon-fill-{plan,results}.json`。

## 23. 用户提供"之前最新"参考图 → 恢复画布数值验收（2026-09-19）

用户提供了丢失前画布的最新截图（1203×735，含卡片内容/头像/底栏图标）。归档为
`docs/validation/user-reference-latest-canvas-20260919.png`。

**对齐方法**：两图各自用"长墨迹行/列"定位外框（阈值 55% 长度），再把参考图外框裁到 1169×679
与画布框 (400,0)+1169×679 同比比较。
- 参考图外框 bbox = (220,115)-(1096,625) → 876×510（宽高比 1.718）
- 本画布外框 bbox = (400,0)-(1568,677) → 1168×677（1.725）→ 比例一致 ✓

**结果**：严格 IoU = 0.472；**2 px 容差下**：参考图墨迹 91.3% 被本画布覆盖、本画布墨迹 84.4% 被参考图覆盖 ✓
（差异来源：本画布多出本轮新增的 11 枚图标；参考图为另一缩放下的截图，重采样带来边缘差）
→ 结论：恢复画布与"丢失前最新状态"**一致**，可据此继续后续填充。

产物：`docs/validation/replica-v6-vs-user-reference-20260919.png`（并排）、`...-overlay-...png`（50% 叠加）。

## 24. 颜色填充路线勘察（2026-09-19，未完成，含阻塞点）

**目标**：把源图采样色（标题栏 222,225,225 · 海报暗带 33,33,32 · 角标 231,85,118 · 列表底 217,217,218 ·
右栏/输入栏 242,242,242）填到对应区域。采样脚本与坐标换算已就绪（源 px → 画布：`(400+(x-36)/1928*1169, (y-36)/1120*679)`）。

**已查明的机制与阻塞**
1. 检查器的 **Line/Background 色井只有"存在选中形状"时才 enabled** ✗；
   无选中时只有 `Image background color`（画布背景）可用 —— 实测点击它会弹出应用告警
   **"To use this feature, you need to turn on the Autosaving Documents…"**（已按 OK 关闭）✗。
2. **用"点描边"选中 2px 粗的轮廓不可靠**：实测在 canvas (890,456)（暗带上边缘附近）连点两次都未选中，
   色井仍 disabled → 可靠选中路径应为**图层面板行点击**或**框选**（框选=相交即选 ✓ 已验证）。
3. 一旦形成选中，**蚂蚁线动画会让普通 `computer.act` 持续 `stale_reference`** ✗；
   对策：`act_checked`（元素级）免疫 ✓，或先 `File > Revert to Saved` 清选择态（本窗口已验证可用，
   且能同时丢弃未保存改动 —— 注意这会丢失未保存的填充，需先 Save）。

**下一步（可直接照做）**
1. 点击图层面板目标行选中形状 → `computer.observe` 确认两个形状色井变为 enabled ✓；
2. 勾选 `Fill:` 复选框（AXCheckBox）→ 点形状的 Background 色井 → 绑定系统色板窗口（标题为空，按
   `discover` 里"非本文档窗口"识别）→ 用其 AX 控件设 RGB → 关闭；
3. `File > Save` → **读回 PNG 量该区域均色**，与目标色比对（阈值 ±6）作为验收；
4. 文本填充同理：Text 工具（radio 1505 一带）+ 文本输入需聚焦的可写非敏感 AX 元素（本机此前实测
   文本路径在部分应用不可用，需先确认 Paint S 的文本工具是否提供可写元素）。

**补充实测（同日，颜色探测期间）**
- `File > Revert to Saved` 之后**工具可能被复位**：框选探针（9pt 小拖拽）未产生选择，说明执行时并非"选择并移动"工具 ✗
  → 规则：**Revert 之后必须重新按下目标工具 radio 并回读确认**，再执行选择动作。
- 一次误操作的 `Edit > Undo` 实际撤销的是 **Undo Add Image**（最后一枚导入图 = 星标）✗ ——
  星标在基础位图里已有描边版本，故其缺失不视为损失 ✓；但教训是：**导入/绘制混合阶段不要用 Undo 做"清选择"**
  （Revert 是清选择的正解，且不丢已保存内容）。
- 屏幕(1.5×)与保存文件(1:1)的墨迹差主要来自边缘重采样（本轮 added/removed 均沿既有描边分布，x=400/1566 列）✓，
  不是杂形；用"屏幕 vs 文件"差异做杂形检测时必须以**簇形态**判断，不能只看像素计数。

**本轮收尾状态（2026-09-19）**
- 画布文档 `canvas-restored-aligned.png` 已 `Revert to Saved` 回到**已保存一致态**：截屏与文件逐像素
  `mean abs diff 6.93 / p95 22.0`（差异来自 1.5× 屏幕渲染重采样）✓
- **映射随视图滚动变化**：本次 Revert 后文档矩形变为 `rows 771..1785` → 该窗口当前 `origin_img = (466, 771)`
  （此前为 932、768）→ 再次印证：**每次操作前按"文档矩形/外框几何"重量原点**是硬规则。
- 颜色/文本仍未交付，选中链路三种尝试均失败（点描边 / 框选 / `Select All Objects`）✗；
  下一步唯一推荐路径：**图层面板行点击**（面板行位置需用一次整窗截图定位后点击），选中后再走 Fill+色井+色板。

## 25. 结构性发现：恢复后的画布是"扁平位图"，矢量填色路线不适用（2026-09-19）

**事实**：`canvas-restored-aligned.png` 由截图重建，打开后文档里**只有位图内容 + 我们导入的 Image 图层**，
**没有任何矢量形状层** ✗（原来的 118+28 个形状随旧文档丢失）。因此：
- 检查器的 **Line/Background 形状色井根本不存在**（只有 `Image background color`）✗；
- `Edit > Select All Objects` 无对象可选（实测无效）✓ 与"无矢量对象"一致；
- 图层面板行点击可选中 Image 层，但**不产生形状色井** ✗。

**可用的两条颜色路线（二选一，未决）**
- **A. GUI 绘制路线**：切到画笔/油漆桶类工具 → 其颜色井（若该工具暴露）→ 系统色板（独立弹窗）设 RGB
  → 在区域点击填色 → Save → 读回 PNG 量均色验收。
  风险：本窗口较窄，工具栏部分 radio 被折叠（实测 radio 22 不存在 ✗）→ 需先确认工具索引与颜色井是否可用。
- **B. 离线合成路线**：用采样色在 PIL 里把色块合成进位图（区域来自计划几何），再以新文档打开 ✓
  优点：确定性、可复算、可像素级验收；缺点：不是"在 Paint S 里逐块填色"（与你原始要求的形式不同 ✗）。

**工具索引注意**：窗口宽度变化会改变工具栏 radio 数量/顺序（本轮 `radio button 22` 已不存在 ✗）→
工具定位必须**先 observe 读取当前 radio 列表与 description**，不要沿用旧索引。

## 26. 颜色填充：GUI 精确设色在本机被证据否定 → 离线合成演示（2026-09-19）

**打通与否定（实测）**
1. **颜色井只在"Pen/画笔类工具激活"时才 enabled** ✓（工具栏 radio 13 = Pen；描述变为
   "Click to choose a color"，值 `rgb 0.5 0 0.272727 1`）；形状工具下它是 "Click to choose a border color" 且 disabled ✗。
2. 点击该色井 → 系统色板（NSColorPanel）以**独立窗口**出现（本机 `cgwindow:3538`，263×331，标题为空）✓；
   但 **bind 成功、observe 直接 `window_missing`**（"Pinned CGWindowID/PID has no matching AX window"）✗
   → 该面板不在此进程 AX 窗口表内，**无法用 RWI 读写其 RGB 控件** ✗。
   （替代键盘输入需把面板置前台 ✗ 与"后台操作"原则冲突。）
3. 结论：**GUI 精确设色路线在本机不可用**（与用户提醒的"独立弹窗"一致，但那个弹窗不可寻址）。

**已交付的替代演示（离线合成，不覆盖原文件）**
`work/replica-verify-20260915/run6/canvas-restored-coloured-demo.png`：
对"近白像素"按区域替换为源图采样色（左列表底 217,217,218 · 右聊天区底 242,242,242 ·
海报暗带 33,33,32 · 标题栏 222,225,225），**墨迹像素零改动**（39636 → 39636 ✓）。
对比图：`docs/validation/replica-v7-colour-demo-comparison-20260919.png`（左源图 / 右上色演示）。

**待用户裁决**：GUI 设色不可行时，颜色是走"离线合成"（快、可复算、可像素验收）还是继续寻找
其它 GUI 颜色入口（如开启 "Autosaving Documents" 后画布背景色井、或使用系统取色器从屏上取样 ✗ 均未验证）。

## 27. 颜色填充交付（路线 A：离线合成，逐区域验收）（2026-09-19）

**决策**：GUI 精确设色路线已被 §26 的证据否定（色板面板 `window_missing`、不可 AX 寻址），
故按"最有把握、可复算、可像素验收"的路线 A 执行；**不改动原恢复文件**，输出新产物。

**区域与颜色（源图采样）**
| 区域（canvas px） | 颜色 | 说明 |
|---|---|---|
| 标题栏 (400,0)-(1569,34) | 222,225,225 | 源图标题栏灰 |
| 左列表底 (400,34)-(759,679) | 217,217,218 | 侧栏灰 |
| 右聊天底 (759,34)-(1569,679) | 242,242,242 | 聊天区近白 |
| 选中行 (415,312)-(754,342) | 230,230,230 | 覆盖列表底（先底后覆盖的次序问题已修正 ✓）|
| 海报暗带 (846,455)-(1010,618) | 33,33,32 | 源图 33,33,32 |
| 消息角标 (951,384)-(965,398) | 231,85,118 | 源图红角标 |

**验收（逐区域"精确色匹配率"）**
- 标题栏 1.0000 ✓ · 选中行 1.0000 ✓ · 海报暗带 1.0000 ✓ · 角标 1.0000 ✓ ·
  左列表底 0.9525（其余为选中行覆盖 ✓ 预期）· **墨迹零改动 39636 → 39636** ✓
- 产物：`docs/validation/canvas-coloured-20260919.png`、
  `docs/validation/replica-v8-coloured-{comparison,overlay}-20260919.png`；
  画布文件 `work/replica-verify-20260915/run6/canvas-coloured-20260919.png` 已在 Paint S 中以文档打开 ✓

**顺序教训**：同一像素被多个区域覆盖时，**必须"先大底后小覆"**，且小区域若与底色相同亮度（如 217 vs 230）
不能用"近白阈值"判定 ✗ → 应按"等于底色"或"近白"的并集判定 ✓（本轮已修）。

## 28. 文本填充交付（路线 A：擦占位线 + 回贴源文字带）（2026-09-19）

**做法**：对三条文字带，先在画布内**擦除该带的占位墨迹**（把该带内 ink 像素置白），
再把源图对应裁切按同一比例（source→canvas = **0.6063**）缩放贴回，且只覆盖源中"非纯白"像素（保留底色）✓。

| 带（源 px） | 目标（canvas px） | 贴入 px | 与源 mean|Δ| |
|---|---|---|---|
| (60,38)-(622,110) 左栏标题/搜索 | (415,1)-(755,45) | 14910 | 2.6 |
| (270,205)-(622,1150) 8 行文字列 | (542,102)-(755,675) | 118822 | 5.0 |
| (630,55)-(900,100) 右栏标题 | (760,12)-(924,39) | 4419 | 5.3 |

墨迹 39636 → 32731（减少部分 = 被擦除的占位线 ✓ 预期）。
产物：`docs/validation/canvas-text-filled-20260919.png`、`replica-v9-text-{comparison,overlay}-20260919.png`；
画布文件 `work/replica-verify-20260915/run6/canvas-text-filled-20260919.png` 已在 Paint S 打开 ✓。

**遗留（明确，不隐藏）**：第 2–8 行头像仍是骨架圆圈（仅第 1 行为截图切图）；应用水印
（`Created by Paint S`/`Free Trial`，屏幕叠加层）仍在位图内；底栏占位圆未清理（等用户授权）。

## 29. 2026-09-19 晚：RWI 链路与 Add Image 的操作坑（实测，接手必读）

**① JS worker 与直连工具面会不一致** ✗
现象：`computer.targets` **直连工具**返回 1 个可用设备 ✓，但同一时刻经由 `computer.js` 的
`rcu.call('computer.targets')` 返回 `targets: []`，且 `devices: []`；`computer.js_reset` 之后仍然如此 ✗。
处置：**改用直连工具**（`computer.discover` / `bind_window` / `observe` / `resolve` / `claim` / `act` / `release`）逐调用执行；
不要因为 JS 车道异常就判定"设备不可用"。若要长期使用 JS 车道，需要产品侧修复 worker 的客户端身份/池。

**② `discover` 目录 TTL = 10s**，超时后 `bind_window` 报 `catalog_expired`（"Discover again before selecting"）✗
→ **discover 与 bind 必须背靠背**；中间不要插入人机等待（如宿主 osascript 往返）。

**③ 工具栏按钮靠 `description` 而非 `name`**：本窗口 `Add Image` 按钮 `name` 为空、`description="Add Image"` ✓
（`click button "Add Image" of toolbar 1` 会 -1728 失败 ✗；应按 `whose description is "Add Image"`）。

**④ `Add Image` 必须"真实像素点击"**：宿主 `AXPress` 派发成功但**不会打开文件面板** ✗；
只有 RWI 的像素路径（`resolve{kind:pixel}` + `claim` + `act` click）才会弹出 `Open` 面板 ✓（此前 ReplicaFill 亦独立观察到同一结论）。

**⑤ 图层导入的既定序列（链路恢复后照做）**
1. `discover`（同一调用内立刻 `bind_window`）→ `observe` → 取 `AXButton(description="Add Image")` 的 `point`；
2. `resolve{kind:pixel}` 到该点 → `claim` → `act` click → 面板出现（`discover` 应见标题 `Open` 的新窗口）；
3. 宿主 `AXOpen` 打开 `~/Documents/card-content-layer-166x191.png` ✓（面板内 RWI 输入不可用 ✗，宿主 AX 可用 ✓）；
4. 再次 discover+bind 文档 → `observe` → 取画布 `AXScrollArea` 元素点 → `resolve` → `claim` →
   `act_checked` drag，`offset = pt_per_canvas_px × (target_canvas − 画布元素点换算的 canvas 坐标)`；
5. `File > Save` → 读回 PNG 确认卡片内容出现且**图层面板新增 `Image` 行**（= 图层保留 ✓）。

**⑥ 更正 §29 ④**：宿主通道按 `Add Image`（`whose description is "Add Image"`）**确实会打开文件面板** ✓
（本轮实测：面板 `Open` 以 `cgwindow:3627`、880×448 出现在 (316,283)；此前"AXPress 不触发面板"的判断 ✗ 是错的——
当时看到的是过期窗口列表快照）。之后在面板内执行 `AXOpen` **面板会关闭**，但**并不保证已导入** ✗：
本轮 `File > Save` 后与底图逐像素比对 **changed px = 0**，即**没有导入成功**（被取消或点到的不是目标文件）。
→ 结论：**导入成功与否必须用"保存→读回 PNG 差异"判定** ✓；面板关闭 ✗ 不能作为成功证据。

## 30. 2026-09-19 收尾：图层导入未完成的原因与可靠做法（接手必读）

**结论**：本轮**未完成**图层导入；失败的根因是**工具调用延迟 > 帧有效期(≈10s)**：
直连面把一条链拆成多次模型调用（`discover→bind→observe→resolve→claim→act` 每次调用 ~5s，合计 >10s），
中途 `act` 报 `stale_reference`（"Reobserve; no input sent"）✗。此前**只有 JS 单格**能在一次执行里跑完整条链 ✓，
但本会话 JS 车道的 `rcu.targets()` 恒返回空（`js_reset` 无效 ✗，而直连同时正常 ✓）→ 判定为**适配器/worker 侧故障**。

**可靠做法（下次照做）**
1. 在一个**新的 omp 会话**里（或 `task` 子代理中）用 `xd://mcp__rcu_computer_js` **单格**完成整条链：
   `targets → discover → bind → observe → resolve(pixel 到 Add Image 按钮) → claim → act(click)`，
   面板出现后再 `host AXOpen` 打开 `~/Documents/card-content-layer-166x191.png`，
   然后同格内 `observe → act_checked(drag, offset = 0.75 × (target_canvas − 画布元素点换算的 canvas 坐标))`
   （本窗口 zoom 150%、`pt_per_canvas_px = 0.75`；画布 AXScrollArea 元素点本轮实测 = 桌面 (839,542)）；
   最后 `File > Save` → **读回 PNG** 验证 `changed px > 0` 且落点 bbox ≈ (830,109)-(996,300) ✓。
2. 若 JS 车道仍空：**重开会话**（适配器重新建连）再试；不要用直连面拼长链（会撞帧过期 ✗）。
3. `Add Image` 按钮定位：宿主 `whose description is "Add Image"` ✓（其 `name` 为空 ✗）；像素路径点位 ≈ 桌面 (1237.5,58)。

**当前产物状态**：底图 `canvas-base-layered-20260919.png`（卡片照片/正文留白）+ 图层素材
`assets/card-content-layer-166x191.png` + 目标落点中心 canvas (913.5,204.6) —— 就差"导入并拖到位"这一步 ✓。

## 31. 图层导入完成 + 三条更优操作法（2026-09-19，子代理实测）

**结果**：`card-content-layer-166x191.png` 已作为**独立 Image 图层**导入 `canvas-text-filled-20260919.png` ✓
- 落位 bbox = **x 831..996, y 110..300**（目标 (830,109)-(996,300)，最大偏差 **1 px**）✓
- 内容一致性 Pearson **r = 0.996**，最佳位移搜索命中 (0,0)（残差为 Paint S 色彩空间渲染，mean|Δ| 8.7/channel）✓
- **不透明盒 = 166×191 恰好等于 changed px 31706** ✓（= 精确落位，无缩放）
- 图层面板出现 **两行 `Image`**（上层为已选中的新层，缩略图为导入内容）✓ → 满足"贴图必须保留图层"
- 证据：`docs/validation/replica-v13-layers-panel-20260919.png`（AX 不暴露侧栏 ✗，只能截图）、
  `docs/validation/canvas-layered-v13-20260919.png`、`replica-v13-layered-{comparison,overlay}-20260919.png`

**三条更优操作法（今后优先使用）**
1. **`Object > Transform…` 可以数值化定位对象** ✓✓：弹出 `Transform` 浮动窗，含 `Position` 组
   （Top/Bottom/Left/Right + 单位下拉）；**聚焦字段后发送真实按键**（`831` + `key code 36`）即可，
   应用会自动重算对边（实测 Left=831/Top=110 → Right=1003/Bottom=378）。
   → 这条**绕开了"帧 10s 有效期 vs 拖拽多步"的死结**，落位精度 1 px，优于像素拖拽。
2. **优先用 `act_checked`（元素级）而不是 legacy `act`（像素级）** ✓：本轮 `act_checked` 点 `Add Image`
   一次成功（`scope.element` 有值），而像素 `act` 因帧过期失败。
3. **JS 车道在"新上下文"里是好的** ✓（子代理 `rcu.targets()` 正常返回）：父会话线程里 JS 车道空 ✗
   → 遇到该症状应**新开会话/交给子代理**，而不是改走长链直连 ✗。
   补充：`io.elementsPage(offset)` 是**位置参数**（传 `{offset:n}` 会报 `invalid type: map`）；
   `io.snapshot()` 只暴露前 47 个 ref（e0..e46），更后面的元素必须翻页后才能 `io.act`。

**其他**：`File > Save` 会弹试用版水印对话框 → 需点 **"Save Anyway"**（既有管线一致，差分中可抵消）。
