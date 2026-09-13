# Mac 受控端：通用 agent 装配

日期：2026-09-13。遵循 ADR 0002，不另建 Mac 专属 MCP 或任务服务。
本批源码已通过独立回归；尚未全局部署或进行 Host Mac GUI 验收。

## 同一条跨端链路

`AI Host → rcu-mcp → mTLS device RPC → rcu-agent → rcu-macos`

- `PlatformPolicy` 增加 `{"kind":"macos","user_uid":501}`；UID 是部署示例，
  不是产品常量。生产依赖仅在 macOS target 引入 `rcu-macos`；MCP/transport
  不引入 AX/CG/UIA 执行后端，通用系统运行时依赖与 GUI 驱动分开。
- `NativeMacDesktop` 的构造、身份和工具目录不要求 PID、窗口、应用名或业务文本。
  窗口关闭仅影响被选资源，不改变 MCP 安装和服务生命周期。
- `discover` 提供当前同用户、屏幕可见的 CG 窗口；8条/12KiB分页、10秒目录期限。
  `bind_window` 用精确 CGWindowID、PID、进程出生及新鲜窗口指纹，不自动抢焦点。
  CGWindowID 指纹不冒称系统提供的窗口出生序号。

## 身份、原生执行与边界

- UID/EUID/CG console UID、实际 `SecuritySessionId`、host/boot、on-console 和
  login-done 事实共同校验；必需字段未知就拒绝。可选硬件 console-set 字段缺失时，
  如实为 unknown；锁域始终用保守的逻辑 console 角色，不因该可选字段变化拆出新锁。
- AX/屏幕/Post/Listen 的非弹窗预检只描述**实际运行进程**。源码测试进程权限通过，
  不代表未来安装路径、签名或其他启动上下文也有相同 TCC 权限。
- 复用 AX、ScreenCaptureKit 和 CGEvent。每个有界输入批次复查身份、租约、取消、
  焦点、布局、敏感元素与命中；按钮/键释放独立清理。
  `CGEventPost` 没有系统接收回执，`system_accepted=null`，须另行验证实际效果。
- 首块仍校验原 frame/ref/revision。单次动作内的后续文字块或第二击使用仅本次
  调用持有的原生 AX anchor，允许自身值/正常聚焦变化，原身份、布局和安全守卫
  不变；不把新状态变成可跨动作复用的输入授权。
- 截图原生回调超时后仍持有 pending 状态和资源；换窗不能抹掉未完成回调。
  恢复输入只读检查 HID/会话按键状态及 agent 写入空闲，不重放旧输入。

## 本批能力，不扩写成全量 Mac 版

| 能力 | 本批范围 |
|---|---|
| 通用 agent、动态发现/精确绑定、工具能力分级 | 源码接入同一协议 |
| AX/截图、点击/键盘/文本/滚动、独立效果查询 | 有底层实现和公开负向回归；真实 Host Mac 验收未做 |
| 自动激活/启动、原生 owner 链迁移、隐藏/最小化窗口发现 | 未实现，返回不可用 |
| 多点拖拽、菱形 Pointer、capture_visual、原生 ROI | 未实现，不从 Windows 能力推断 |
| 锁屏/多显示器跨屏/硬件身份保证 | 未证明，不绕过平台边界 |

当前未全局部署 Mac agent、未改现有 Windows 配对、未修改 TCC，也没有借 Shell
客户端冒充 Host 原生 MCP。后续必须完成精确安装件/进程权限、真实应用与同协议接入验收。

独立回归：默认424通过/5忽略，全feature 511通过/6忽略，fmt、严格clippy及
架构门禁通过；详细范围见[本批验证](../validation/2026-09-13-macos-agent-foundation.md)。

身份依据：[Apple 安全会话与快速用户切换](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPMultipleUsers/Concepts/FastUserSwitching.html)、
[CG console-set 字段](https://developer.apple.com/documentation/coregraphics/kcgsessionconsolesetkey)。
硬件字段与逻辑冲突域的分离是本项目的保守实现设计，不是 Apple 的硬件拓扑保证。
