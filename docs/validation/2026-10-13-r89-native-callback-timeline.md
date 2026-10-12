# R89：原生拖放后台→前台逐回调验收（2026-10-13）

## 独立原生结果：本次专用场景通过

源PID3838 / CG46394 → 目的PID3839 / CG46395，均在已确认的BetterDisplay虚拟屏。当前Host加载typed computer.drag_between，唯一请求r89-native-pair-timeline-1，5000ms，无全局键盘fallback、无重放。

| 阶段 | 实际结果 |
|---|---|
| 原生开始 | native sequence19，source app_active/key/frontmost=false |
| own actor | began后1950ms before activate，1959ms after activate，1963ms before order，1974ms after order；此时同native19仍active，API请求不等于完成切换 |
| 首次前台native moved | began后2026.512ms，native_sequence=active_sequence=19，app_active/key/frontmost=true |
| 继续 | 共79次原生moved：30后台、49前台，同一会话，无第二source请求 |
| 原生结束 | began后4957.958ms，同native19，operation1 Copy，cancelled0 |
| 另一进程接收 | prepared/performed/concluded/consumed=1；external=true；payload精确对应source3838/run_id r89-native-source/offer1/native drop sequence19 |
| 释放／清理 | 原请求terminal/resources_released，无pending/uncertain/fence，durable replay forbidden；两应用自身End控件关闭，实际wrapper退出0/0，两侧attachments清理 |

源118条、目的20条timeline，丢弃均0。process_monotonic_ns仅本进程原序，不跨进程直接排序；unix_ms是独立样本，不宣称原子clock。MCP原receipt仍goal_verified=false；业务成功来自接收端独立回读，不改写receipt。

**R88取消结果保留。** 本次新增fixture诊断与采样／持久化可能影响时序；这次成功不证明R88根因已找到，也不证明生产后端已修复或所有应用可持续后台拖放。没有生产Agent/MCP部署，服务PID80314与四权限保持。

## 修改、回归与副本回滚

- fixture-only Oracle::note_timeline、Phase、TimelineEvent：256条FIFO、total/dropped计数、unknown保留null，不改变native session/consume/input语义。
- 真实source/destination callbacks接入；ended先记录真实native seq和active seq，再清空。own actor将activate与window order的前后阶段分开记录，无RefCell借用跨这些reentrant API。
- check-only诚实声明native_timeline实现/容量，但native_callbacks_observed=false且不创建GUI、不发送输入。
- 同命令、同输入：BASELINE descriptor probe1（原无timeline），MODIFIED0，独立ROLLBACK副本probe1，REAPPLIED0；fixture自身退出均0。5个源文件和实际binary原hash全部恢复。仅副本回滚，生产未回滚。
- 最新源fmt、workspace807/0/11ignored、receiver89/0、clippy所有target/features locked offline、架构ratchet及10规则测试均0；ratchet不是架构已完成。
- 实际GUI `.app` binary SHA c8e678b39ba56e3293af663f1e52340a04d196aa2a339d9919072e1338b65551；回归后的target/debug binary为另一哈希7a26aab1，不把它冒称此次GUI运行对象。操作员校验脚本的hash假设与变量覆盖错误已保留、修正，不重放输入。

## 按时间顺序的实际原始截图

### 之前：目的窗口

![之前：目的窗口](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r89-native-timeline-2026-10-13/01-destination-before-original.png)

### 之前：源窗口

![之前：源窗口](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r89-native-timeline-2026-10-13/01-source-before-original.png)

### 之后：目的窗口

![之后：目的窗口](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r89-native-timeline-2026-10-13/03-destination-after-original.png)

### 之后：源窗口

![之后：源窗口](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r89-native-timeline-2026-10-13/03-source-after-original.png)

## 下一项：已证实的多窗口发现缓存覆盖

R88发现source后82ms再发现destination，首次attach source返回window_not_discovered；相同source原生身份重新discover后attach通过。代码每个desktop只有一个Catalog，replace会用新查询替换全部rows；04最新catalog通过TTL检查后缺source行，不是catalog_expired。当前R89采用discoverA→attachA→discoverB→attachB，无此错误。

产品应补有界、context-local多snapshot registry，独立TTL及总row上限，保留已观察exact PID/window/epoch selection，并继续bind时原生process birth/geometry/display/session重验。禁止无期限union、刷新旧查询TTL、按title替换或输入重放。后续仍包括冷后台hover/键盘、foreign-provider层级、浏览器多窗口、正式offscreen Host及分层性能；全F01–F16未完成。

证据目录：/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/native-callback-timeline-r89

## 分层耗时，非性能优越性结论

实际Host工具 computer.drag_between 5421ms，native begin→end4957.958ms、输入轨迹5000ms；整个接力任务417468ms。三者作用域不同，未测native CPU，不将整个任务等待／模型／导出耗时归入原生执行，不将R88/R89非受控比较当性能改善。本轮原图由父loaded MCP响应对象直接落盘，避免模型逐字复制PNG/base64和巨大转义shell。
