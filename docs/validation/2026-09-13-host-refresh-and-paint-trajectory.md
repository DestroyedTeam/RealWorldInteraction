# Host 刷新、Paint 连续笔画与逐点检查成本

用户重启 Codex 后，在原任务中验证；不是另起 Shell MCP 客户端。
GUI 操作全部通过 Host 加载的 `computer.*` / Rust QuickJS `computer.js`。
SSH 只部署 Rust agent、检查进程/哈希，不绘图、不伪造 GUI 验收。

## 入口恢复

- 当前 `computer.act_checked` 工具 schema 已包含 typed `drag`，JS catalog 含
  snapshot/summary；运行态返回连接复用计数。旧 `unknown variant drag` 消失。
- Windows agent 独立存活；重新发现/绑定 Paint PID 2024 / hwnd:2e0636。
  从未重用旧 context/frame/lease。用户移动了窗口，后续新观察以新几何为准。
- 旧 agent 上首个 400ms 工具栏轨迹真实进入 native，但 3 次移动后迟到中止。
  查原 execution 确认 terminal/resources_released；不把 ACK 或释放当目标完成。

## 发现及整改

1. 加入操作内的 guard phase 总计/最大耗时；失败也保留，不记录用户文本。
   原生诊断中两轮 exact UIA scope 共 233534µs、最大 224697µs；最终
   Windows 状态检查共 1531µs。这里明确定位逐点 UIA，而不是猜测网络或模型。
2. 精简输入专用祖先遍历，避免对每个中间父节点取名称、矩形等观察属性；
   将 root 与所选元素身份在一次新鲜遍历中证明。仍为每个采样重新验证，
   root 32 层/1500ms、目标 8 层不变；敏感/启用属性在遍历后读 Current 值。
   键盘、文本、观察路径不改。无跨点缓存授权、跳过焦点或取消检查。
3. **上述优化独自未让 16ms 网格通过。** 800ms 工具栏实验仍在 8 次移动后
   中止；Paint 400ms 绘图实验在 10 次移动后中止。没有隐藏这两次失败。
4. 核心采样器支持有界 16..64ms 插值网格，默认仍为 16ms；Windows 明确
   advertise/use 64ms 开发节拍。所有调用方顶点及其时间原样保留，密集顶点
   不会被删掉；250ms 迟到阈值、逐点 guards、一次 down/release 规则不变。
   这是减少内部插值密度以匹配同步检查成本，不是宣称已优化到 60 FPS。

UIA 属性跨进程调用及缓存行为的依据：
[Microsoft caching](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-cachingforclients)、
[parent traversal](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationtreewalker-getparentelementbuildcache)。
原生新鲜查询和真实耗时仍需现场验证，不能由文档推导其延迟。

## 失败恢复不是重放

- 每次不确定结果保留稳定请求 ID；查询原 execution/durable reservation。
  真实 release-only cleanup 得到确认；持久不确定记录在 agent 重启后仍保留。
- 一次诊断在 fence 存在时被入口拒绝，无 native dispatch。之后显式
  `computer.recover_input` 独立检查 native writers idle / pressed keys empty，
  清理资源 fence，但不让旧操作重新可执行、不认定业务目标完成。
- Paint 半条线的 grounding SHA 与失败后记录完全相同，才通过新鲜 Undo 控件
  撤销自己这条失败测试线。独立截图确认恢复空白，未清空/覆盖用户文档。

## 成功的真实 Paint 检查

最终开发版 agent PID 9900 / Session 1，实例
`45ad5026-ebb0-47f0-8b99-ee4ca90160e5`；SHA256
`18005b0977317e421aeb9d500d8e583886530e6a2b9386cc53e52474e240ffdb`。
只替换 exact PID/path/birth/hash 对应的 agent，保留各次 rollback；配置、
SSH、防火墙和启动任务定义不变。Host 不需要再重启才能使用新的设备采样能力。

请求 `cadence64-paint-closed-stroke-20260913-r1`：

| 节点 | 实际结果 |
|---|---|
| 真实软件/输入 | Paint 的 Pencil；从图像确认在画布中的原生窗口中心开始，一次 checked drag |
| 路径 | 5 个必经顶点，0/160/320/480/640ms，4 条边回到起点 |
| 原生执行 | 13 samples，12 moves 全部插入，release=true；698ms |
| 资源 | terminal/resources_released，lease released，无 fence，无重放 |
| 效果 | 后续真实截图显示完整闭合菱形；不是由插入 ACK 推断 |
| Pointer | 14 次更新请求、最终 idle/visible=true、停留在末点，error=null |
| 人工实屏 | 用户明确回答“看到了跟随和停留”；不扩大为流畅度/FPS评价 |
| 最终 PNG | `0b08ce32ee3bfbe6666a55d3bf090652642f50301a92472786cb603c6343f5d7` |

本机原生预览：`outputs/drag-guard-20260913/native-paint-closed-stroke.png`。
grounding 截图仍临时隐藏 Pointer；指针外观/跟随的人工实屏证据与此截图分开。
本次是闭合连续笔画测试，**不是 NameSix 完整图标交付**，没有保存/重开验收。

## 耗时与剩余工作

- 成功 cell 7901.80ms：前置 observe 1681.15、claim 1.93、act_checked 5240.14、
  release 2.07、后置 observe 914.14、image 58.82ms；result 是 adapter-local。
- act 的 5240.14ms 不等于原生笔画的 698ms。后者中 13 轮 exact-scope 共
  531183µs、最大 71963µs；最终 native-state 共 13971µs。
- 工具外间隔包含推理、调度、构建和部署，不能叫纯思考时间。本轮不是无干扰
  A/B benchmark，不能拿总耗时或不同路径的样本宣称性能倍数。
- 最后连接计数为 131 writes / 111 reused checkouts / 20 authenticated
  connections，**累计包含本轮多次 agent 重启**，不是一次笔画开了 20 条连接。
- 当前只证明新版 Host 入口、受保护的完整笔画、释放、连接复用和可见跟随。
  64ms 是约 15.6Hz 的内部插值网格，必经顶点可能更密；不是呈现 FPS。
  16ms 下的 UIA 同步成本、完整动作预检成本、高帧率反馈、反馈入图的独立只读
  通道、多笔编排、现场 Stop/焦点变化及完整图标仍是未完成项。
- UIOS 仍只是本开发版的设计依据，不声称正式签名/发布验收或 Computer Use 等价。

## 回归

- fmt check、workspace locked/offline：277 passed / 3 ignored，34 suites。
- workspace all-targets/all-features Clippy `-D warnings` 通过。
- Windows ARM64 release / all-targets/all-features cross Clippy 通过。
- 新测试覆盖 8/32 层边界、root 与 target 必须同时证明、每次样本无旧授权、
  所有必经顶点/时间保留、非法采样节拍拒绝及新节拍下 down/moves/release。
- all-features workspace locked/offline：364 passed / 4 ignored，38 suites；
  显式隔离 fixture worker，不将其算作真实绘图或 Host 原生入口证据。

原始 native MCP 回执、部署前后哈希、测试/构建日志保留在
`work/drag-guard-20260913/`，不把运行身份、截图或二进制提交 Git。
