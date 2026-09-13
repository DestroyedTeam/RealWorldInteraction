# 普通／管理员、连续绘制与停止：本轮证据

状态：持续验收，不是全项目完成。GUI 全部经当前 Host 加载的原生 RCU MCP；SSH 仅用于部署和独立取证，没有 Shell/Python GUI 客户端。

直观看过程：[35 步实际截图](../../outputs/acceptance-20260913/README.md)。每张保留原生 PNG，不补画、不修图。

## 第二次重启后的收口

- 新Host返回新增transport字段，独立只读verifier也确认当前入口可用。Windows agent
  重启后，首次targets内部只做一次Describe重连，`BrokenPipe → reply_validated`；
  原笔画durable记录前后逐字节相同、`replay_allowed=false`。
- 实际调用指定1200ms截止；cell约1274ms返回，原生轨迹约863ms／13次移动后取消并
  抬键。父级查明terminal、无native pending、resources released后，一次释放租约成功。
  这次没有context_busy，未自动清除围栏；显式恢复核对空闲与按键后，原uncertain历史仍不变。
- Paint窗口发现完整且为空，另经成功CIM查询确认旧PID2024已不存在；MCP服务仍available。
  后续最小化Explorer恢复缺口已实现并实测；详情见[显式窗口激活](../architecture/explicit-window-activation.md)。
- 原生GUI从Explorer打开系统Run界面，填写mspaint与原PNG路径后启动新Paint，
  PID7372／出生标识134337780956635793；不是SSH启动或Shell MCP客户端。
  文件仍12676字节／`d155aa4f…`，512px画布切片仍`ab04a812…`。

新截图28–35是本轮；30是关闭确认框，不冒充进程退出截图。1200ms输入参数取自实际
工具调用并有明确转录，非独立抓包。1274ms与863ms都不等于整轮模型思考或屏幕显示延迟。

## 已实际确认

| 验证点 | 结果 |
|---|---|
| 重启后的工具加载 | `computer.capture_visual` 确实进入 Host 并返回 Paint 截图，不只是磁盘安装成功 |
| 普通／管理员部署 | 普通监听 PID 2436：Session 1、非 elevated、integrity 8192；恢复管理员 PID 9788：integrity 12288。均为原生 token 测量，不拿任务 RunLevel 或 SSH 身份代替 |
| 原配置保留 | 原 agent 配置、启动任务、证书、私钥的字节和 ACL 未变；临时普通任务和配置已清理 |
| 连续两笔 | 一个 Host JS 单元、一个租约内，两个 640 ms 设备轨迹；动作间重新观察，约 5.33 s 后释放。原生 512px 切片确认两个闭合菱形 |
| 截止取消 | 1200 ms JS 截止；原生轨迹在约 560 ms、7 次移动处停止，抬键插入成功。原执行查询为 terminal、无 native pending、resources released；保留部分笔画、不重放 |
| 显式恢复 | 原生写入空闲、254 个键／鼠标状态核对通过后解除该资源围栏；原不确定输入历史仍禁止重放 |
| ROI 输入 | 新 agent 的局部观察 `[1269,712,512,512]` 产生引用，局部点 `[130,330]` 映射桌面 `[1993,1229]`；约 739 ms 的动作调用画出左下第三个闭合菱形，随后释放成功 |
| 缩略图细线 | 最近邻预览丢细线；面积平均预览保留本次闭合轮廓。升级前后原生切片 SHA 相同，证实不是重画导致变化 |
| 原画重开 | GUI Open 对话框输入原 PNG 路径；系统确认后只丢弃本轮有截图的 Untitled 测试草稿。新观察确认原图重开，原文件仍12676字节、SHA `d155aa4f…` |
| 原画独立像素对照 | 首次切片与基线相差2px，位于鼠标悬停位置附近；通过原生点击Pencil移到工具栏后，512px切片SHA为 `ab04a812…`，与本轮步骤10完全相同。未通过修图隐藏差异 |

ROI／面积采样已部署 Windows PID **11876**，进程出生标识 `134337701190355262`；可回滚的开发版，不声称正式 UIOS 验收。

## 耗时不混算

- 两笔约 **5.33 s** 是批次执行时间，不是完整模型回合时间，也不是与 Computer Use 的公平 A/B。
- ROI 示例：observe **955 ms**，act **739 ms**，后验 observe **602 ms**；前后截图与动作分列。
- 同画布 1024px 预览：旧调用约 **157 ms / 126,380 bytes**，新调用约 **170 ms / 140,669 bytes**。新采样提升细线可读性，编码／采样开销增加；单样本不支持“更快”的结论。
- 原生 512px 图像保持完整；极小比例的任意细线可辨性、屏幕显示延迟和整体模型理解耗时仍需专项对照。

## 失败也保留

1. 首次普通部署的临时配置用了继承 ACL，未通过既有私有文件合同，未派发 GUI 输入。恢复原服务后，用新建样本和普通权限 Rust 诊断复现 `agent_private_acl_not_user_only`。复现不是遗失的原始 stderr。
2. 新配置在 **CreateNew 时**使用原有精确 FileSecurity；不通过修改旧 ACL、放宽 Rust reader 或 ExecutionPolicy 来过检。
3. 普通 Fill 的即时回执成功，但即时截图尚未变化；后续截图才显示 Fill 已选。未把 ACK 当视觉完成。
4. WinUI 菜单焦点／树的完整 runtime ID 不一致时鼠标被拒绝；未截断 ID、同名替代或 Invoke。另一次新鲜焦点上的独立 Enter 才创建本轮测试画布。
5. agent 重启后旧 Host 有两次 Broken pipe，新只读查询才恢复。没有重发输入或重复启动。
6. 截止取消的首次父级租约清理返回 `context_busy`。该错误保留；Rust 有界清理修订尚待新 Host 实测。
7. 重开操作的调用方最初忽略 discover 的8项分页，漏看Open弹窗；旧主窗口被正确阻止输入。用精确PID+已知标题过滤并确认完整覆盖后才绑定。另有一次SDK未保存观察的本地拒绝，未派发输入。
8. 丢弃本轮草稿后，首次观察遇 `native_scene_changed`；没有重放已成功的点击，新的只读观察确认原图已打开。这类异步窗口转换不能按输入失败重试。

## 仍在推进

- 新连接池／有界清理、整个应用退出后服务健康与GUI重开已按上节实测；历史旧Host失败记录不改写。
- 菜单精确指针目标提示、真实人工介入／异常窗口重用、公共 UIA 观察开销、Mac 通用受控端与剩余架构债务，继续跟踪完整实施计划。

## 最终离线门禁（独立 verifier，Darwin arm64）

- fmt、workspace clippy（all-targets/all-features，`-D warnings`）：通过。
- 默认 workspace：**381 passed / 4 ignored**。
- all-features workspace：**468 passed / 5 ignored**；使用当次构建的公开 fixture worker，SHA `cd92c812…`，不是旧 worker。
- Trace：**12 项**，包含取消清理在途 Drop 不重复释放；连接池/ROI/面积采样各有独立定向评审和回归。
- 架构 ratchet：0 violations，仍有 **11 个 permitted hotspots**；不等于完成架构治理。
- 源码／manifest／ratchet 共218文件运行前后摘要相同：`dd9d101da2a1bc2ea536b077f8b6b10f83f50317e6714589db209a6f926ae820`。

未设置 `RCU_NATIVE_TEST_WORKER` 的首批 all-features 曾失败（缺公开前置，后续suite未运行），原日志保留。未修改测试绕过；按公开说明构建 worker 后才完整重跑通过。以上离线结果不替代 Windows／Host 实机验收。

## 原始证据定位

本机 `work/resume-and-performance-20260913/`：

- `config-acl-live-reproduction.json`、`privilege-r04b-*.json`：部署、失败及恢复。
- `continuous-fullframe.json`、`two-strokes-native-tile.json`：多笔动作和像素后验。
- `cancel-host-cell.json`、`cancel-durable-query.json`、`cancel-resource-recovery.json`：取消／终态／恢复，三者不能互相代替。
- `roi-area-install-idle.json`、`roi-area-install.json`、`roi-area-reconnect-reads.json`：新版 agent 部署与旧连接失败。
- `pre-area-current.json`、`area-preview-live.json`、`roi-native-stroke.json`、`roi-native-operation.json`：实际预览、ROI 动作和终态。
- `final-gate-verifier-r2-command-log.json`：精确命令／环境、计数和日志索引。
- `mcp-resume-install.json`、`mcp-after-file-replace-runtime.json`：磁盘新版本和仍在运行的旧 Host，分开留证。
- `reopen-*.json`、`reopened-original-*.json`、`original-tile-comparison.json`：原生文件对话框／草稿处置、重开、SSH文件哈希与2px差异调查。

二进制、凭据和原始截图不提交 Git。整体状态见 [完整实施计划](../product/implementation-status.md)。
