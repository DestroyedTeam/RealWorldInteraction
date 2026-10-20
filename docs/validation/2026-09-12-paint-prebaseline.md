# Paint 前置实测：不等于 Codex 原生入口验收

日期：2026-09-12。用户指出入口和 Pointer 验收不符合要求后，已停止终端测试
客户端；未继续派发 Windows 输入。Paint 保留未保存的中间画稿。

## 范围必须纠正

本轮绘图由终端中的 Node 测试客户端启动**实际已安装的 Rust MCP 进程**，通过
标准 MCP/持久 JS → mTLS → Windows Rust agent → SendInput/GDI/UIA 执行。
Python 用于构建、部署、读取测试记录和组织调用者代码，不是产品 GUI 后端。
但这仍然**不是用户要求的新 Codex 任务直接调用已加载原生 MCP 工具的验收**。
不能以底层确实经过 MCP 为由将两种入口等同。

Windows Paint 的启动是已授权 SSH 部署/准备动作；绘图、颜色、尺寸和填充才是
实际 MCP 设备调用。两条通道不能混记为全部通过 GUI。当前调用任务的旧 adapter
进程未热替换；新安装二进制由上述独立测试客户端启动，不能称为新 Codex 任务。

## 已落地的通用改动

- `b15cfe9`：设备传输 client/server 拆分，独立于行为变更。
- `c467ecf`：ALPN 协商、独占连接复用、独立工作/控制容量、不重放。
- `8b77bcc`：桌面授权的发现不再错误依赖上一次选择窗口的 input grant。
- 上述提交已推送 `engineering/governance-native-mcp`。

实际已安装 MCP 进程连续完成 12 次设备请求，计数为 2 条认证连接、2 条协商
keepalive 连接、10 次复用、12 次已验证回复。独立 SSH 查询也观察到对应 agent
持有的两条 TCP 连接。后续在绑定 Paint 后仍可发现前台 Explorer，证明发现范围
没有再被 Paint 的输入绑定错误缩小。复用计数不是输入成功或模型响应速度证明。

离线：fmt 通过；默认 workspace 264 passed / 2 ignored；all-features workspace
351 passed / 3 ignored（连接复用版本）；all-target/all-features clippy 无警告。
Windows ARM64 交叉构建和发现修复的真实运行另行验证，不能以 Mac 单测冒充。

## 真正做了什么、哪里没通过

| 节点 | 结果 |
|---|---|
| Paint 11.2605.81.0 启动、发现、绑定、截图 | 已实际执行；启动使用 SSH |
| 原生前台切换 | SetForegroundWindow 返回 false；未重放，用户手动点前台 |
| 512×512 画布、Fit to window、Pencil、Fill、调色板 | 已通过 GUI 操作并读回/截图 |
| 银色盾牌、顶部弧形、橙色 X | 已在 Paint 实际绘制，平面近似稿 |
| 连续路径 | 盾牌、内轮廓、顶部弧形和 X 的有界路径成功；不是逐点 MCP |
| 断裂线 | 原生 UIA 查询中断，只有部分笔画，不算通过 |
| 取消/失效处理 | 失败笔画未重放；先查原执行，再做原生空闲资源恢复检查 |
| 目标图标完成、保存、重新打开 | 未完成；不是最终交付图 |
| 可见 Pointer 跟随/动作覆盖层 | 未实现，不以系统鼠标移动或截图替代 |
| Mac Computer Use 对照 | 转入用户要求的新任务，尚无完成绘图对照 |

调用者还犯过一次名称匹配错误：实际控件名是
`Using Fill with color tool on Canvas`，不是推测的名称；该错误前只选中了 Fill，
没有完成对应填色。之后重新观察并按实际控件选择，没有重放已完成工具选择。

纯像素引用另一次在输入前因 `stale_reference` 被拒绝。工具栏提示等动态画面
是候选原因，尚未定位为唯一原因。后续调用者增加了原生观察，并保留完整图像
约束；没有禁用校验或将拒绝伪装成成功。这增加了观察开销，仍属于效率缺口。

## 真实中断与原执行状态

`paint-fracture-outline-1` 返回 `uia_pointer_unavailable` / `0x80040201`：
轨迹已进入 native dispatch，197 ms 时中断，10 次 move 已插入，计划 161 个
sample，release 插入成功。原执行查询随后返回 terminal=true、native pending=false、
resources_released=true。恢复检查实际读到 pressed keys/buttons=[]、native writers
idle，清除该 domain fence，明确保留 old_operations_replayable=false。
没有撤销、重试或继续这笔未完成轨迹，画稿仍含部分断裂线。

该 HRESULT 对应 UIA_E_ELEMENTNOTAVAILABLE。Microsoft 说明 ElementFromPoint
可能在命中元素于返回前消失时产生此错误，并建议正确处理，包括再次执行查询。
这支持“受限的只读重新查询”作为待验证方向，不证明 Paint 中具体哪个 provider
对象消失，更不授权重复 SendInput 或跳过焦点、作用域、敏感元素检查。
[Microsoft ElementFromPoint](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation-elementfrompoint)。
本轮尚未实现该重查策略。

## 完整时间，不能只报 RPC

Paint 消费者记录从 09:45:13.777 UTC 到 10:04:50.273 UTC：

- 整段墙钟：1176.496 秒，即 **19 分 36.496 秒**。
- 19 个 JS cell：合计 101.844 秒。
- 245 次内部 MCP 工具调用：合计 101.672 秒，包含在 cell 时间中，不能再相加。
- cell 之间：1074.652 秒。包含模型规划、查文件/截图、错误调查、其他工具及
  用户介入；**不能全部标成纯模型思考时间**。
- 这些时间不含此前构建/部署和 Paint 启动，也不是受控跨 OS A/B benchmark。

因此“热连接调用约 2 ms”绝不等于整轮绘图流畅。本次整轮耗时长、仍未完成
图标，用户对入口、交互输出和 Pointer 的反馈成立。

## 证据位置与下一组

本机临时原始证据：`work/keepalive-20260912/` 中的
`live-mcp-evidence.json`、`paint-mcp-evidence.json`、`paint-mcp-trace.json`、
`drawing-15-0.png`（断裂前）、`drawing-18-0.png`（中断后）、部署/构建/测试日志。
原始图片、运行配置和身份材料不提交 Git。

按用户最新指令新建独立任务：`01a09515-0d5f-7d01-870d-4d575680924d`，
标题 **Mac Computer Use 绘图基线验收**。只允许内置 Computer Use 操作正式
Paintbrush，记录实际 API、连续/离散动作、光标、输入/观察/纠错/保存和整轮时间。
“六组”是用户希望覆盖的维度，不臆造为官方固定六个 API。
该任务完成/阻塞证据返回前，不形成“RCU 已替代 Computer Use”的结论。
