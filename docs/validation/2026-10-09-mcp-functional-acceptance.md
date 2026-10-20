# 2026-10-09 新 MCP 整体实机验收

## 当前结果

不是只验动态小地图。实际加载39工具（含computer.hover）；四权限仍有效。限定virtual display4自有fixture，不给游戏发输入、不启动或停止接力。随后沿用稳定签名部署已完成回归的Agent，当前PID **73141**、instance **de7cc66c-8a96-4730-8f4d-a569b711e483**，MCP磁盘/当前Host工具表未改，不需要为此次Agent更新再次重启Codex。

| 更新/功能 | 本轮实际结果 | 结论 |
|---|---|---|
| 新工具加载/协议 | 本会话39工具，typed hover接受schema；新stdio同样39/2025-11-25 | 通过加载；不等于hover effect |
| 快照/原图清晰度 | 默认1320×1104原生tile、2×mapping，真实PNG保存；169/209/152ms等单样本 | 此fixture可读，不推广所有App |
| 窗口发现/独立attachment | A43125/cg14476、B43126/cg14475；两个固定target、同一物理lease，未替换desktop或Live源 | 通过 |
| place_window | A到virtual UUID13E4E9B0…/CGbounds1824,24,660,552；471ms/原生placement+rendered确认 | 通过；新鲜重观察后输入 |
| typed AXPress/cached controls | A/B独立计数0→1，controls保留ID/point、cache query0ms；AX/readback+接收端验证 | 通过，不能称指针click |
| typed完整drag | A实收1down/8drag/1up、completed1、无orphan；500ms路径adapter684ms | 通过此配置；app-active曾被primer改变，OS-frontmost仍不同，不称全部后台/输入连续性 |
| short输出/timings/details | A显式耗时197ms/文本832bytes；B默认无timings/文本634bytes，完整cache180ms/terminal/已release | 通过；字节不是token、不同调用非公平性能benchmark |
| 跨窗引用 | 向B提交A的新frame被unknown_frame拒绝，0native，计数各1不变 | 通过，lease清理保持 |
| standalone hover | 310ms及254ms返回returned，但view moved/enter/tooltip均0；未按回执计成功 | **未通过效果验收**，下一步接收队列分层探针 |
| Live实际采集/显示器 | session2采集363/呈现362，readback bounds2656,120,1040,912，on_requested_display=true，用户确认页面出现 | 通过显示/采集；不是原生Host task-ID attestation |
| Live源消失/过期 | fixture600s自己closed后image清空、paused/process_identity_unknown；300s Live结束后hidden/worker_done；stop正常 | 通过这两种清理；非截图保护 |
| Live任务切换/运行中Hide/Resume | 首次120s会话在可见前到期；之后动作采样时已到期，缺乏运行中的中间回读 | **仍待单独验收**，不把用户返回或stopped API回执当效果 |
| 服务更换/客户端重绑/不重放 | 新Agent身份实际经loaded MCP取得；旧execution_ref_expired，无自动fallback；显式durable query仍原回执/replay forbidden | 通过；过期不推断无效果 |
| 更新后真实输入 | 新fixture78394/cg14751 safe_count0→1/366ms、RWI close120ms、state.closed/exit0/无timer close，随后detach | 通过，非只stdio |

## 实际顺序与问题

1. 先启动此前pending receiver40152（120s）；typed hover310ms，view原生tile坐标500,360→logical250,180，明确hover区域，但接收0。等分析后fixture已正常timerclosed/exit0；此时点击process_identity_unknown、未派发，随后操作方误从失败snapshot取view，产生missingview参数错误及find TypeError。错误保留，不归为随机工具失败、不重放请求。
2. 改为有界600s的两窗口A/B，实际sampled exec hashbabb4902…；移A到上方virtual区域，B保持原位置。独立计数/drag/跨窗负例依次完成。A drag时app_active=true只代表回调时采样；hover没有回调，不证明之后仍active。
3. Live首先open_in_codex回queued，120s过期时0frames；第二次300s重新开，后续用户确认「Real World Interaction：Mac端」页面出现，native实际363/362及virtual bounds确认。源先600s关闭，随后Live本身过期清理；不能把后续stopped状态的hide/resume当运行中效果。
4. A/B均自身windowWillClose记录closed、controller各exit0，Live stopped/worker_done，两个target detach。未signal/强杀，更未借游戏测试。
5. 在cooperative native lease排除下核查旧instance56条durable记录terminal/no-pending/released/no-uncertainty，原子交换Agent；旧PID66480退出、签名/read-only就绪均0。证书/配对/CA/config/profile/MCP保护哈希不变，不clear fence/recover，不宣称HID全局idle或永久独占游戏。
6. 新Agent安装exec SHA **e1f0b7bf33aeb9394fa2845acb007a5da4f79245444fc25623384b9dcf35a0d4**（sign-script产物，与先前候选4c4d…的签名包装不同；unsigned source04d4a33f…相同）。四权限true/no prompt，旧执行引用拒绝+显式durable原回执验证，不重放输入。另新fixture真实点击/关闭通过。动态原点/hover效果未因此声称修好。

## 真实截图（按顺序，未编辑）

![初始hover receiver](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/host-mcp-acceptance/hoverSnapshot0.0.png>)

![A真实drag完成](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/host-mcp-acceptance/overallAfterDragA.0.png>)

![更新后的counter1](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/host-mcp-acceptance/postReloadSnapshot1.0.png>)

## 待补齐与后续完整计划

- hover需要自有NSApplication/NSWindow sendEvent透明日志（正常super），分传输/窗口route/view tracking三层。现在没有该probe，不能由0view断定CGPost未到app。保持no button/no warp/no global fallback/no activation要求，不把不明subtype3作为默认公开API。
- 继续任务可见性变化中实时采样、运行中Hide/Close/resume；之后keyboard/scroll专用可逆fixture、同应用多窗/系统panel、occluded/offscreen/多框架状态、cold/warm/P50/P95、并发/回收/模型输出。原50repo/150正文与16feature矩阵仍保留，不收窄为本表全部已完成。
- 本轮没有新增产品Rust行为；部署的是上一轮642passed/11ignored/41suites、fmt/Clippy/release均0的既有候选。本轮这些离线门禁是既有结果，不冒充重跑；新的结果是本会话真实MCP验收与部署事件。

## 三态与证据

同一只读fresh MCP命令/输入，副本 BASELINE38→MODIFIED39→主ROLLBACK38→重施39，各exit0/原binary hash恢复；stdio不是Host GUI验收。Agent bundle副本完整旧文件hash恢复、strict同DR0、重施全文件hash一致；不对生产服务做回滚。

主四角色继续沿用10/06事务。完整工具trace/图片/oracle/命令：
/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/host-mcp-acceptance

原生部署、实际identity及保护哈希：
/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/agent-hot-reload-r1
