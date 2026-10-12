# R63：显式 SkyLight 后台移动通道与实机对照（2026-10-12）

## 结论

新增严格选择 `MovePointerOptions.delivery=background_skylight_experimental`，仅 macOS、仅单次 MouseMoved。保持默认 public PID 路径；缺符号／能力／原生身份时拒绝，不自动切前台、不重放、不获取认证材料，不写已证实会破坏时间戳的 field 58。

**真实窗口投递已验证，通用冷态后台悬停尚未通过。** 不能把私有符号可用、void 调用返回或窗口收到了事件当作 tooltip 成功。

## 顺序实测

所有 GUI 输入来自本任务已加载 RWI MCP，只操作 Virtual4 的专用 Receiver 144／145。目标144为 PID2534、CG44392；实际 Quartz bounds[3036,504,660,552]，测试点[3136,674]、AppKit window-local[100,382]。对照只改变明确选择的投递通道与已记录的前台／物理指针状态。

| 顺序 | 操作／实际接收 | 独立原生结果 |
|---|---|---|
| 1 | 明确 focus peer145；目标144 inactive/nonkey | 目标 hover enter/move/tooltip 全0；物理框架点不在目标区域 |
| 2 | 默认 public background move 一次 | NSApplication／NSWindow 各接收 type5 一次，正确CG44392／hover_zone；view／tooltip仍0 |
| 3 | opt-in SkyLight background move 一次 | 各再接收一次相同type5／窗口／点；view／tooltip仍0；框架mouseLocation仍[1924,495]，转换到目标[-1112,382]，区域外 |
| 4 | 明确 focus144，再 foreground_system move 一次 | 物理框架点变[3136,495]并转换为[100,382]；真实enter1／move1；稍后tooltip请求1 |
| 5 | 明确focus145，保持已建立的指针／tracking，再发新的SPI move | 目标inactive/nonkey／fg2571；真实move1→2；这是既有tracking条件下成功，不是冷态后台悬停证明 |
| 6 | 分别明确 AXPress Close145、Close144，detach | 两进程exit0、expiry请求0；不靠超时冒充Close成功 |

143是此前已启动的前置窗口：既有物理指针已在hover，先有enter1／tooltip1；没有向它发RWI输入，480秒到期exit0。保留该原始结果，不重新启动或归因给本轮SPI。

## 实际截图（原始PNG，不重绘）

下面是按真实时间顺序的两张截图。第一张立即前台移动后的tooltip计数仍0；稍后原生dwell记录为1。第二张为既有tracking下的后台SPI移动后，move2／tooltip1。不存在“后台冷态成功”截图。

![前台移动正对照](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r63-explicit-skylight-pointer-2026-10-12/01-explicit-foreground-tracking-control.png)

![既有tracking下的后台SPI移动](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r63-explicit-skylight-pointer-2026-10-12/02-inactive-pretracked-skylight-move.png)

## 部署、回归与错误保留

- 同一稳定代码签名身份部署新Agent：PID2300、instance509117ee-6f8a-4638-99ab-73a03ab31b5d。实际已加载MCP返回实验路由available=true。四项权限仍true；CA、profile、配置、已安装MCP哈希不变，无需Codex重启或人工授权。
- 11项完整门禁exit0；workspace774 passed／0 failed／11 ignored，Receiver70、Live UI7。fmt、locked/offline工作区测试、all-target/all-feature Clippy-Dwarnings、release与架构门禁均实际执行。
- 初始enum直接比较导致编译exit101；改为其数值表示。额外timestamp声明const类型不匹配warning已修正。新增源接线断言受fmt换行影响，首次完整测试exit101；改为函数调用标识，重新完整11项门禁。原始错误记录保留，未覆盖。
- focus145之后旧144 frame被核心按同一desktop失效，resolve返回unknown_frame／native_execution=false；后续缺reference调用在schema阶段拒绝。改为同一次调用内fresh snapshot→resolve→claim→act，并检查返回reference；没有重发任何已经投递的输入。
- 9个原始输入请求经durable operation逐一回读：全部terminal／resources_released、无pending／cleanup uncertainty／replay。输入acknowledgement不是应用效果证明。
- 源配置同输入BASELINE1／MODIFIED0／ROLLBACK1／REAPPLIED0，独立副本恢复原哈希；不是生产Agent回滚，不是输入重放。四角色沿用既有集合并扩展。

## 范围与下一步

本轮没有宣称后台通用hover已修好，也未证明其他应用、原生DnD、全离屏／其他Space／最小化窗口兼容或性能优于Computer Use。Live的既有320×303点、eye.slash／xmark及真Host任务切换验收保留；未伪造Host组件presence。

继续以真实接收侧证据补齐通用tracking；同时改进focus后失效范围的工具反馈，避免模型持旧frame走进可预防的失败。全计划仍未完成。

参考：[trycua固定提交的SPI实现](https://github.com/trycua/cua/blob/0a10f6964b727cd8ae45cd118a1d392815cdb8b7/libs/cua-driver/rust/crates/platform-macos/src/input/skylight.rs)、[AppKit tracking area文档](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/TrackingAreaObjects/TrackingAreaObjects.html)。这些是实现／平台资料，不是本机应用效果的替代证据。
