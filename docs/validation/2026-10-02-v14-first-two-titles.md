# V14 接续：前两行独立文字源（219对象）

RWI修复部署后继续复刻；GameHub/游戏留在内建主屏，本任务未发送游戏输入。Paint旧PID14404已失效，新PID724。RWI最初未发现目标文档；Computer Use只读显示Paint“最近文档”面板，确认源未打开。Host终端以系统`open -g -a Paint S <已有源>`后台恢复文件；**启动不是RWI lifecycle验收**。恢复后的绘图、取色、字号、面板/文档放置、文字、位置微调和保存均来自当前加载的RWI MCP，不使用终端/其他语言作为GUI输入桥。

## 按时间顺序的实际操作与证据

1. 先核对原工作源与215对象阶段源同SHA `b0fd831f…`。文档加载时一次`place_window`的observe报`capture_binding_changed`，`native_dispatch_entered=false/input_domain_fenced=false`；没有移窗。完成加载、新观察后，独立新请求将PID724/CG4228实际放至BetterDisplay UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`，bounds **`[1824,24,1220,527]`**，独立discover确认。启动时提示面板也跟随移动；未激活/移动游戏。
2. [215对象原生基线](../../outputs/validation/2026-10-02-v14-resume/00-before-215-list.png)与既有`work/v14-ref420.raw`逐像素对照。新窗的正常工具canvas原点是原图 **`[386,258]`**，列表tile为`[786,258,420,680]`；Text属性工具栏展开时canvas y为284。首次基线crop用了`[785,257]`，比较时仅对基线ROI采样+1,+1对齐，不改GUI作品像素。不能继续沿用旧窗口`1000,390/418`映射。
3. 用既有Fill实际值`rgb 0.902268…`（与旧遮盖同一NSColor、原生平坦区`#E6E6E6`）、Border off，新增两块局部Rectangle遮旧标题。**每次画图后Paint自动回到Select and Move**；最初第二次drag返回但并未遮掉旧标题，独立截图发现，重新明确选择Rectangle后才新增第二块。最终归档只增加预期2个Rectangle，没有把回执当成图形成功或保存多余试笔。
4. Text字号通过RWI精确combo焦点、cmd+a、type_text、return实际回读为**17**。取色首次落在字形抗锯齿灰，面板实际`#929292`，未设置为文字色；从实心笔画重新取样后实际显示**`#010101`**，才用`Set as Text Color`。CG标题为空的picker4247仍按fresh PID/window/epoch绑定，原生8controls确认其Picked Color身份；每次在主屏出现后显式移至虚拟屏空位 **`[1824,575,341,367]`**，再设色、关闭。一次动画时AX/CG边界差1–2pt导致`window_binding_ambiguous`，保持拒绝/未派发，稳定后新发现处理，未放宽边界。
5. 新建独立Text **“Docode-Vip售后①群”**和**“Docode-Vip售…”**；后者键入的ASCII三点被Paint正常智能替换为归档中的U+2026，真实源已核对。第1行初次baseline高4px，用选择对象后精确Canvas `down` 四次下移4px；每次都是新snapshot/新request，不微拖缩放柄。第2行直接按当前映射落位。
6. RWI `cmd+s` 实际写入正式工作源。纯色空白处的一次取消选择因动态选框使全窗内容变更，在Core派发前报`stale_reference`，diagnostic仅content changed、input未发、域不围栏；没有重放。改为显式选Rectangle清除编辑覆盖层，得到[219对象干净原生列表](../../outputs/validation/2026-10-02-v14-resume/01-final219-list.png)与[2000×679完整原生画布](../../outputs/validation/2026-10-02-v14-resume/01-final219-canvas.png)。保留纯色区严格全窗规则。

## 源文件与像素比较

[219对象阶段源](../../outputs/paint-v14-source/paint-v14-20261002-rows1-2-titles-editable.paint)与正式工作源逐字节一致：**503829 bytes**，SHA256 **`2fc9d4cd8b2c0deccd758a0efce9f113425843e8f6fdfdd9b1245af29f685e94`**。分类12 Image/57 Rectangle/99 Line/23 Ellipse/9 RoundRectangle/**19 Text**。新四对象依次Rectangle/Text/Rectangle/Text；两标题的实际归档字体为`.AppleSystemUIFont`及中文fallback，均17pt。旧215阶段快照保留。

独立只读归档审计发现AppKit在重新保存时将14个此前原生Text的系统字体alias解析为实际字体名（如`.AppleSimplifiedChineseFont-Regular`→`.PingFangUITextSC-Regular`），并重算AutoResize的缓存Bezier bounds；不能声称原215对象全部字节/元数据不变。其余201个原对象在递归展开UID、规范化NSDictionary后语义相同；**已配准的420×679列表范围内，排除两标题ROI，变化像素0/RGB绝对差0**。原12个Image未替换/合并；Undo历史跨应用重启持久性仍未证明。[manifest与归档细节](../../outputs/paint-v14-source/paint-v14-20261002-rows1-2-titles-editable.manifest.json)。

[3倍参考/215之前/219之后对照](../../outputs/validation/2026-10-02-v14-resume/02-reference-before-after-titles-3x.png)来自实际原生像素，不是合成作品。

| 固定区域（420×680列表内） | 215 MSE | 219 MSE | 降低 | 参考bbox | 当前bbox |
|---|---:|---:|---:|---|---|
| 第一行标题 `[130,118,312,149]` | 5388.32 | 3347.67 | 37.87% | `[147,126,297,141]` | `[147,126,306,140]` |
| 第二行标题 `[130,190,277,224]` | 3950.13 | 1582.91 | 59.93% | `[147,201,262,216]` | `[147,201,266,215]` |

两标题已有独立可编辑对象，位置与误差改善，**仍不是1:1**：第一行宽9px、第二行宽4px，字形/行高/抗锯齿有差异。下一步优先实际GUI试验typeface/字号宽度，再处理两行preview/date；头像灰底、旧酒红轮廓、其他图标和右侧布局继续保留计划。

当前Agent PID92469/instance `08ac04cf-a444-403d-9839-8933259b245d`；四权限保留，输入域false；Paint为Rectangle、无编辑覆盖层/取色模态。该GUI阶段未再修改Rust或部署；之前部署修复全仓537 passed/10 ignored及fmt/Clippy门禁已过。未Git提交/推送。
