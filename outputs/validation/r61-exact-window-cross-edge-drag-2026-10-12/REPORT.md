# R61 · 精确窗口后台拖拽与前台物理命中正反例

当前：RWI / 部署修正后真实跨屏边界拖拽与前台命中检查通过 / 泛化后台悬停等全计划剩余项继续。

## 实际执行顺序与问题

1. **134不是跨屏canvas验收**：原 `--boundary-right` 只让右侧scroll viewport偏出屏幕，canvas仍全部在屏内。本轮未把这个更容易的场景算作cross-edge drag通过。明确AXClose，expiry0/exit0。
2. **仅补测试几何**：新增 `boundary_canvas_right/Rect::place_boundary_canvas_right`，让原样canvas跨越Virtual4右界；不改view、AX角色、firstResponder、input接收或绘图oracle。parser同命令真实BASELINE1→MODIFIED0，check-only不创建GUI/状态；两个离线测试通过，原quarter模式不变。
3. **135暴露真实缺陷**：同PID还出现Colors/Fonts；首次错误选取第一行只进行了观察，没有输入，随后依据已分配CG44120纠正。请求点[3676,829]，应用级hit却指向另一窗口矩形[3205,740,470,270]；原拖拽在发送前拒绝，down/move/up均0。原请求查询保留、未重放；135与peer136后来生命周期到期expiry1/exit0。额外面板出现原因未被本轮证明。
4. **修正产品 `point_element`**：只有几何有效且CFEqual原生关系属于精确目标窗口的application hit可直接使用；其余只独立查所选窗口的有界native child tree，不保留/授权foreign hit。仍验证所选窗口/process、native对象、geometry、分类、modal、lease和no-replay，不改delivery；树选Window不获得空canvas的application-hit证明。
5. ** foreground route不继承后台证明**：新增 `require_foreground_pointer_hit/foreground_hit_matches` 并接入foreground_move::guard；系统光标仍需要真实screen hit的owner、CFEqual window与point包含性，未知或另一窗口明确拒绝。不是删除几何guard。
6. **137实机后台蓝线**：RWI明确focus自有peer138，让137 inactive/nonkey。一次新拖拽从global[3676,829]到[3781,829]跨越3720边界；原NSResponder真收1down/16drag/1up、completed1/orphan0。18个真实callback均active=false/key=false/frontmost=29079/ownCG44174。canvas原图出现蓝线；局部实际start[196,79]→end[301,79]吻合window/canvas变换。不是receipt returned冒充目标成功，也不是continuous/hardware-human证明。
7. **同一137前台正反例**：明确focus137后，同点screen路线实际返回foreground_hit_unproven，native_dispatch=false；没有另走后台。对该窗口另一个有效物理点只发一次foreground move，真实hover enter1/move1，随后tooltip请求1，证明新guard并非一律拒绝正常前台操作。
8. **清理**：137/138各一次AXClose，expiry0/进程0；所有固定bindings已退役。9个原durable请求均terminal/released/no pending/no release uncertainty/no replay。没对其它应用内容发送输入。

## 原始时间顺序图像

### 01 原134几何，不冒充canvas跨界

![01 原134几何，不冒充canvas跨界](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r61-exact-window-cross-edge-drag-2026-10-12/01-original-quarter-window-not-canvas-crossing.png)

### 02 原135拖拽前，原请求拒绝

![02 原135拖拽前，原请求拒绝](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r61-exact-window-cross-edge-drag-2026-10-12/02-original135-before-refused-drag.png)

### 03 修正后137，后台一次拖拽前

![03 修正后137，后台一次拖拽前](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r61-exact-window-cross-edge-drag-2026-10-12/03-new137-before-background-drag.png)

### 04 修正后137，跨界蓝线和完成计数

![04 修正后137，跨界蓝线和完成计数](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r61-exact-window-cross-edge-drag-2026-10-12/04-new137-after-background-cross-edge-drag.png)

## 回归、部署与完成范围

- 最终完整11门exit0：workspace769/0/11/42；Receiver68；Live UI7；fmt check、全targets/features Clippy-Dwarnings、release与架构门通过。稳定同证书部署Agent28828/e57ec955，四权限仍有效，CA/profile/config/安装MCP未变，无需Codex重启。
- 源macOS skill添加“不要选windows[0]”及两种grounding差异，quick_validate实际成功；没有覆盖当前Host加载的cache或捏造已热更新。
- 沿用四角色，源配置匹配输入验证BASELINE1/MODIFIED0/独立副本ROLLBACK1/REAPPLIED0，精确恢复pristine hash；不是生产Agent回滚或原生输入重放。原135拒绝及scope错误观察仍保留。
- 原目标持续保留：前后台切换/Command+Tab、模态/非模态、多窗口、Live真实Host观察、各MCP工具的完整验收及标准PR发布。当前只有本报告具体case被新验到，未将F01–F16全标为complete。generic后台tracking/tooltip、whole offscreen/other-Space/minimized、mixed-DPI、跨应用DnD等仍未验收完成。之前R47/48/56等真实结果保留，不重新演绎成物理用户证明。
