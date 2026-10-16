# RWI MCP：Codex重启后验收（2026-10-08）

## 验收结论及范围

**本Codex会话实际加载的MCP默认View点击封装修复已确认生效。**不以独立stdio进程代替Host验收：两条通道分别记录。没有绑定/操作游戏或其他真实窗口，没有截图、claim、act、输入或权限请求。这里是transport/缓存/封装兼容的无输入验收，不是GUI点击业务效果认证。

## 实际顺序

1. 复用已成功的controller命令从安装路径启动独立MCP；binary SHA256仍为`e5c1a8741e04b21f4e38b010c63649e96c7c20d25a22384e26cbf5929ef0d369`。MCP2025-11-25、38tools、mac-host available、结构化结果与JSON text一致、0images、退出0、stderr空。
2. 调用本会话**真正加载的**`computer.targets({device_id:"mac-host"})`：platform=macos、Agent PID1585、四项输入/辅助/屏幕相关权限reported true，requests_permission=false；原始desktop没有选择窗口，32字符result_ref。本次describe未读取窗口列表或截屏。所有identity/capability仍以原始回执为准。
3. 使用该未绑定desktop和三个唯一、从未签发的view_id，通过真实typed `computer.click`分别发送默认、显式false、显式true。未知view在cache lookup直接失败，早于原图/定位/claim/act，因此不是对任意桌面坐标实际点击：

| 输入 | 当前实际code | adapter elapsed | 实际stage | 结论 |
|---|---|---:|---|---|
| 默认（不传red_marker_cluster） | unknown_view | 7ms | resolve_view一次 | 已经过旧字段schema问题，正常拒绝未签发view |
| explicit false | unknown_view | 1ms | resolve_view一次 | disabled optional正确省略 |
| explicit true | invalid_arguments／unknown red_marker_cluster | 2ms | resolve_view一次 | 当前旧Agent不支持opt-in，明确拒绝，没有降级 |

这些毫秒是三个缓存负例的adapter墙钟，不是GUI/native操作性能benchmark。

4. 用三个details_ref调用本会话`computer.result`：均返回原始错误和stage、goal_verified:false、metadata_sha256；32字符ref，native_dispatched=false。default/false的Agent原始回复还含native_execution:false/result_kind=cached_read。没有失去原始错误/安全状态；也没有把缓存读取伪装为原生成功。

## 验收中发现并修正的Agent源码缺口（尚未部署）

生产`Executor::invoke`的resolve_view原来虽然解析red_marker_cluster，但始终构造Locator::Pixel；独立legacy Server已有正确分支。这意味着未来按当前源码升级Agent时可能静默降级，不能因当前旧Agent明确拒绝就宣称候选全链路正确。

本轮只改`crates/rwi-agent/src/executor.rs`：true选RedMarkerCluster，default/false选Pixel，original_point映射仍保留；Core不支持时明确red_marker_anchor_unavailable，不回退Pixel。实际Executor+synthetic PNG/view三项回归，旧语义harness BASELINE exit101（2pass/1fail，true得到普通Pixel reference），MODIFIED exit0（3pass）；未知view两路仍提前拒绝，default/false映射保真。不放宽像素/焦点/身份/权限/租约防护。

统一门禁：fmt exit0、workspace604passed/10ignored/41suites exit0、all-targets/all-features locked offline Clippy -D warnings exit0。Agent修复只是源码候选，未部署、未签名、未重启Agent；当前运行旧Agent继续拒绝true是如实能力边界。MCP binary没再改，本轮无需再次重启Codex。

## 未验收内容

真实GUI可逆点击、动态小地图邻近动画、hover/drag/focus/业务goal仍未验证。已询问是否分配独立Chrome空白测试窗；没有答复不视为授权，也不让negative未知view变成真实动作。后续将包装层schema与原生像素守卫分开测，不根据central_max_delta=0自动判误拒或调宽阈值。

## 证据

本轮沿用原四角色；新增post-restart-acceptance-2026-10-08与executor-marker-supplement只扩展主事务。真实Host工具请求/回执：implementation-2026-10-08/POST_RESTART_LOADED_MCP.json；独立进程：POST_RESTART_INSTALLED_SMOKE.json。具体BASELINE/MODIFIED/ROLLBACK及原哈希恢复、重构/reapply以VERIFICATION实际事件为准；没有在live root执行rollback。
