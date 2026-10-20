# RWI 后台基础与 MCP 可靠性：实现轮次 2026-10-08

## 范围与实际状态

本轮基于50仓库/150技术正文/16项能力矩阵，在主仓库实现；生产后端和MCP仍为Rust。没有以跨项目源码、Python或AppleScript替代backend。没有操作游戏/飞书/Chrome、输入、截屏、权限调整或Agent重启。下面是离线原生构造、认证TLS组件和workspace回归，不是全后台应用验收，也不是高并发容量或生产性能认证。

## 按实际推进顺序

1. **协议兼容 P0**：定位`computer.click`的View路径无条件传`red_marker_cluster:false`。新增`view_resolution_args`：默认/显式false省略optional字段，true保持显式让旧Agent拒绝。没有失败后重发、strict降级或放宽原生guard。红基线是保留旧语义的builder提取＋回归断言，不是旧源码本来有这些测试：exit101（2失败），修正后新增5项通过。
2. **输出可读性**：`compact_action_reply`只省略原始数据根本没有的key，原生显式null、`goal_verified:false`、operation_ref、cleanup/no-replay、scope/point均保留；错误和uncertain仍走完整路径，原始metadata可通过details_ref读取。缺字段fixture JSON130→89bytes，不拿这个数字声称实机token收益。保持`structuredContent`及序列化JSON TextContent兼容形式，符合[MCP结构化结果规则](https://modelcontextprotocol.io/specification/2025-11-25/server/tools#structured-content)；不是承诺所有模型理解率已提高。
3. **并发控制 P1**：`invoke_device`的operation/cancel不再等待volatile context锁，完整引用仍由Agent验证。普通调用的context等待可被Host取消/EOF打断；旧control错误不能清新context，普通旧错误只有token一致时才失效，且不会等另一次初始化。挂起OpenContext的TLS fixture在旧实现两项超时/exit101，修复后控制查询与EOF通过，无Prepare/Execute。
4. **并发刷新 P1**：`inspect_device`把Describe使用的previous token保留，只有当前仍为该token才应用迟到响应的invalid/instance/targets。原逻辑迟到invalid会重复OpenContext、迟到valid会把新targets覆盖；两个barrier fixture红101→绿0。五项control/race组件测试全部通过，保留单OpenContext互斥，不重放输入。
5. **缓存路径可维护性/占用**：`metadata_fingerprint`一次JSON编码用于size与SHA；`make_room`统一byte/count/TTL准入，`project`用take转移原始树，少一次原始clone。不改变64项/64MiB序列化字节预算/300s与不续期规则，保留原始hash/分页/future/error。原presentation9通过/1忽略，修改后11通过/1忽略。字节预算不等同实际heap测量；没有宣称CPU/内存百分比或吞吐提高。
6. **F01 原生探针**：仅`cfg(all(test,target_os="macos"))`新增`nsevent_probe`，公共AppKit/CG构造→桥接→snapshot→roundtrip；不注册新production route，不调用post、不解析实际window/display，不创建UI。Retained、CFRetain/CGEvent Drop及autoreleasepool边界有测试。基线exact filter为0tests/exit0；新增4tests/exit0。

## 探针实际发现：不能当作后台目标成功

- six fixture cases（down/dragged/up × window0/1048573）所读type/windowNumber/flags/timestamp/button/clickCount/eventNumber往返保留，synthetic windowNumber不是目标身份。
- pressure量化：0.5→127/255，0.625→159/255；公共CG setter的0.75→191/255，不是无损float。
- factory drag delta为0；公开CG setter17/-9→NSEvent17/-9。下一轮必须维护完整gesture的source/button/delta/down/up状态，不能假设位置变化自动生成delta。
- local→global桥接含隐式AppKit坐标上下文；这不是外部window几何或on-screen/occluded/offscreen路由证明。
- 首次create约2984ms、首次属性读取约12863ms，后续构造/读取显著不同。只是该native test进程阶段记录，未定位到单getter，不能推断production latency或软件之间性能优劣。
- 两次探针修正前exit101保留：opaque CGEventRef objc2编码问题；setter pressure无损断言不成立。修正公开pointer编码与量化容差，没有关闭签名检查或放宽目标guard。

## 本轮统一门禁（实际执行）

- `rtk proxy cargo fmt --all -- --check` exit0。
- `rtk proxy cargo test --workspace --locked --offline` exit0：601passed /10ignored /41suites。
- `rtk proxy cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` exit0。
- `rtk proxy node --test integrations/javascript/rwi-client.test.mjs` exit0：50passed。

编译/测试墙钟含编译和Cargo锁等待，不当native/MCP/模型耗时比较；workspace测试日志有缓存编译warning回显，当前源码和独立Clippy门禁未因此修改其他Agent文件。

## 未完成项与下一阶段（完整计划继续保留）

- RequestContext取消锁等待、普通context_unknown响应与新token竞争：当前代码路径有保护，尚需专门端到端负例，不宣称已全覆盖。
- 对旧Agent optional的显式opt-in仍可能拒绝；后续补正式capability/version协商，不能靠静默降级掩盖。
- 自有AppKit receiver fixture：分别记录PID/window/view/client消费、goal readback和前台输入连续性；需要明确分配GUI窗口后执行。
- F04完整source/drag、F05typed Move/Hover、F08/F09独立window-local capture/authority、F07focus恢复仍未交付。SPI隔离候选继续独立，不默认替换。
- 真实应用兼容与同状态A/B、分层耗时/并发负载/峰值资源测量尚未执行。既有Live/多窗口/配对续签/MCP成功率等完整计划不被本轮几项修复替代。

## 证据与事务

复用原四角色，扩展`implementation-2026-10-08`补充；baseline字节单独保存。补丁重构、独立copy rollback/reapply、matching Rust fixture行为、部署与fresh MCP process结果以同事务VERIFICATION实际记录为准。没有在live root上做rollback。各子agent补充证据只扩展主角色，不代替它们。

## 已执行部署：仅MCP adapter

默认feature release构建exit0后，实际启动candidate进程并使用已有mac-host配对profile，仅initialize/tools/list/targets。随后atomic replace已安装MCP，再从安装路径新起进程复验：协商MCP2025-11-25、38工具、typed click schema、mac-host available、structuredContent与JSON text一致，退出0、stderr空、0images；没有capture/input。

- 安装路径：`/Users/vincentadamnemessis/Library/Application Support/RealWorldInteraction/client-v1/bin/rwi-mcp`
- 原binary SHA256：`4bd4eedef223a2fd1137668a2b856f30bac150fc3c7fee849ba861bd44e486ef`
- 新binary SHA256：`e5c1a8741e04b21f4e38b010c63649e96c7c20d25a22384e26cbf5929ef0d369`
- profile hash前后相同；未修改Agent.app/证书/私钥，未重启Agent。
- 磁盘替换不刷新已加载的Host/MCP进程；现有Codex会话需要重启后再做在线typed验收，不冒称已自动切换。部署rollback仅在独立binary copy上执行，不停用户会话。
