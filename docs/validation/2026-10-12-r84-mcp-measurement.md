# R84：发布、分层 MCP 测量与下一原生范围

## 已执行测量
同一已安装9fe5ae6d…Rust MCP，3个独立stdio process；每个100次resources/read、30次tools/list、5次仅mac-host的authenticated Describe/targets。三个process均exit0、stderr空、43工具目录与v3源码完全一致。无capture、输入、Host presence或选窗。

计时从operator写入stdin至同id回复JSON解析完成；单位ms。是该机器/当时负载的本地roundtrip，包括operator/OS调度，不是设备原生CPU、网络链路各段单独时间或通用性能优势。

| spawn→initialize | read median / p95 (n100) | tools median / p95 (n30) | Describe median / p95 (n5) |
|---:|---:|---:|---:|
| 14.926 | 0.183 / 0.292 | 1.101 / 1.469 | 21.089 / 35.801 |
| 14.288 | 0.207 / 0.341 | 1.153 / 2.841 | 32.420 / 46.259 |
| 13.133 | 0.206 / 0.338 | 1.163 / 1.581 | 18.308 / 63.168 |

实际Host双PID Live的59/147帧、来源省略/清图/关闭另记；不得以这三次stdio测量替代Host GUI。现有native placement442ms、两次close105/101ms为各工具adapter total，与Live capture last_capture_ms及controller间隔分开，不做语言优劣结论。fixture生命周期、离屏图像转换、真实Native input/client stress仍是独立范围。

## 已发布结果
- 7个reviewable commits分别对应fixture、compact Live、multi-source/retained capture、volatile facts、MCP UI/capability、operator packager、验收文档；均实际提交并推送。首次HTTPS push128保留；仅HTTP/1.1 bounded correction成功0，不force-push、不关闭TLS。
- PR1已ready for review（isDraft=false、OPEN），公开仓库可读。源码head e83d38bdf1d413feefd16b2141c5c825db8e583e的真实CI [38261800260](https://github.com/DestroyedTeam/RealWorldInteraction/actions/runs/38261800260) macOS/Windows两job及fmt/tests/all-feature Clippy/architecture/negative tests全部success；不是Windows native桌面验收。
- 此后文档补测量时不修改Rust/UI/packager源码；后续doc-only head的CI仍必须按该head回读，不冒用旧head状态。

## 下一项与完整计划
F12显式双PID与真实Host v3生命周期现在已接受，但不是自动foreign owner或所有菜单/窗口族。F13原生cross-app DnD、F14独立browser content route及F05/F06/F07/F08/F09/F10/F15/F16未验部分继续保持原范围。

F13的下一实验须先以两个明确分配的own-app source/destination建立typed payload、原生session/取消、destination实际消费与两端birth/context/lease/no-replay证据；不能把现有canvas蓝线或任意post receipt当成DnD。这是待实施要求，不是宣称当前支持。Apple明确区分NSView启动session与source结束回调，且dragging pasteboard是session属性：[NSDraggingSession](https://developer.apple.com/documentation/appkit/nsdraggingsession)、[beginDraggingSession](https://developer.apple.com/documentation/appkit/nsview/begindraggingsession(with:event:source:))。本地要求 docs/product/requirements-and-acceptance.md、当前F13状态与原R61跨屏canvas/R62–R64 tracking记录一并复核；冻结CrossMacro reference只作为对照，未写入或运行其旧bridge。

无人工配合、VM唤醒、浏览器/游戏/既有文档操作或新权限；不把现有非完成字段改为全量通过。下一candidate必须遵守现有owned/native身份与不重放，不让生产启动/工具发现依赖业务stage。
