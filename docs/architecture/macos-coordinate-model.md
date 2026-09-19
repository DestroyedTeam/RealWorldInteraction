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

## 6. 根因（2026-09-15 实测确证）：窗口图像含"非等比 chrome 带" → 线性映射失真

- 实测同一窗口两次 observe：
  * 图像 `3600×2156`、窗口 `1800×1078 pt` → 2.000× ✓ 纯缩放（换算正确）
  * 图像 `3600×2262`、同窗口 → 2.098× ✗ **图像比 2× 多出 106px 带**
- 现实现 `pixel_to_desktop = rect.map(image_size)`（逐轴线性）在第二种情况下
  **整幅图像被拉伸到窗口 rect** → 图像内所有点的桌面换算产生纵向偏移
  （106px 带被摊到全高 → 越靠下偏差越大）→ 一切"图像像素→点击"都会点偏。
- 结论：**参考系不是"窗口 rect"，而是"该次捕获实际的区域"**。

### 修复清单（按层）
1. **rcu-macos（核心）**：捕获时记录并上报**捕获区域**（窗口内/全局 pt 的 rect，
   含 chrome 带的实际偏移）；`pixel_to_desktop` 必须由该 rect 与图像尺寸推导，
   而不是由窗口 rect 推导。若捕获含 chrome，则 chrome 带要么裁掉，要么在映射中体现。
2. **rcu-core/工具面**：`Scene` 增加明确的捕获区域字段（如 `capture_region_pt`），
   并在 `computer.observe` 回执中可见；`Locator::Pixel` 以图像像素为唯一输入，
   换算全部由 RCU 完成（agent 不做任何全局换算）。
3. **拓扑变化的显式化**：窗口记录的 `native_epoch` 摘要加入"所在显示器 + 当前活动
   显示器集合（id/bounds/pixels/main）"；显示器热插拔后旧 frame/reference 一律失效
   （fail-closed 并提示重新 observe），不依赖 agent 察觉。
4. **JS 车道**：允许清单加入 #3a/#4/#5 新工具（pointer_feedback / move_window /
   pointer 相关），使 JS 与 MCP 工具面一致。
5. **测试**：为 1 与 3 增加离线单测（纯函数：给定 rect/图像/拓扑 → 期望映射与失效）。

### iShot 复核（修复后由用户执行）
- 用户在全屏截图（1:1）中标出鼠标位置；RCU 侧把同一"图像像素目标"映射出的
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
1) RCU 在画布内落一笔显著深红墨迹（并给出"按实现换算的期望全局点"）；
2) 用户全屏截图（系统截图/iShot 均可）发回；
3) RCU 在截图中量出墨迹的实际位置（按显示器 origin/scale 换算为全局 pt）与期望比对，误差 ≤1pt。

## 9. 用户实测真值（iShot 读数，2026-09-15）

用户对 RCU 所画深红线段用 iShot 读出：
- 起点 **(492, 669)**、终点 **(526, 670)**（线段长 ≈34pt，几乎水平）
- 用户结论：**微信截图 / iShot / 其它截图携带的坐标都准确；只有 RCU 截图与 Python 截图坐标漂移。**

推断与算式：
- iShot 读数很可能是**显示器局部坐标**（内建屏原点在其左上角）。若如此，全局坐标
  = (282+492, 1152+669) = **(774, 1821)**。
- RCU 当时对该笔迹的期望（按实现换算）= 窗口原点(282,1190) + 图像px(1800,1300)/2
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
  而 RCU 期望全局 y = 1840 ✗ 差 −188 ✗（与"y 仅差 19"的早先推算矛盾）
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
  RCU `coords` 探针同期读光标 = 一次配对，实测吻合早先 PIL 推导 (729,1609) ✓）。
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
- rcu-imaging：复用现有 PNG 解码（`view()` 已解码并缩放）→ 新增 `diff_bbox(a, b) -> Option<[u32;4]>`
  （逐像素比较，容差可配；只返回包围盒与比例，不回传像素）。
- rcu-core：`Engine::ink_delta(...)`（校验两 artifact 属于同 target/frame 系，字节上限沿用 budget）。
- rcu-agent：executor 分支 + 目录/能力注册（writes: false，纯读取）。
- rcu-mcp：`#[tool] computer.ink_delta` + 参数；JS 车道同步放行。
- 测试：rcu-imaging 纯函数单测（构造两张小图 → 期望 bbox）；离线可跑。
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
 
Open 面板批量导入的失败根因已验证：面板按最近文件排序，固定 `group 1` 不一定是请求文件；面板保持 modal 后，后续 RCU 输入全部 `native_dispatch_entered=false`。修正为：
1. 删除本轮临时 `~/Documents/icon-tdesign-*.png`；
2. 每次只复制一个目标 PNG；
3. Add Image → AXOpen `group 1` → radio 19 → `act_checked` 移动 → 回读状态栏；
4. 下一枚前重复清理。

按此 SOP 已完成输入栏 5 枚腾讯 TDesign MIT 图标：
- `folder`: `(880.8,460.5)` → 状态栏 `X=881,Y=460`；
- `image`: `(932.1,460.8)`；`mail`: `(974.5,460.8)`；`microphone`: `(1016.6,460.8)`；`time`: `(1527.2,460.8)`。
目标/偏移及 RCU request receipt：`work/replica-verify-20260915/run5/bottom-icon-isolated-results.json`；整窗证据：`run5/bottom-icons-isolated-final.png`、`run5/icon-fill-final.png`。
更新对比：`docs/validation/replica-v5-icon-fill-{current,comparison,overlay}-20260918.png`。

当前图标已填：头部电话、videocam、TDesign share、TDesign add；输入栏 folder、image、mail、microphone、time；剩余 header grid/chevron/more、底栏 smiley/scissors/people/screen/robot、星标等待后续，文本与颜色仍未填。

## 20. 2026-09-19 重启后的画布重建与坐标自证

**事实**：本机重启（`security session 100002 → 100003`、boot 时间戳变化、Paint S 83334 → 2578，rcu-agent 进程随之退出）。原文档
`white-canvas-2x.png` 是夹具文件且未保存，磁盘与窗口列表均已不存在 → 当日复刻画布丢失；仓库内的全部计划（118/28 形状）、
素材（头像、卡片内容、10 枚图标）与 SOP 未受影响。

**恢复步骤（已实测，全部走既有授权通道）**
1. `pgrep -fl rcu-agent` 无进程、`agent.pid` 记录 50652、端口 45129 未监听 → 用 `hub start` 以 `--config agent.json` 重启部署件，ready 条件 `port 45129 + log listening`；
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
