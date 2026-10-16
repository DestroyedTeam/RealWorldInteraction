# 2026-10-08 重启验收与 F05 接续

## 当前结论

- 上轮新MCP的成功耗时输出已由本会话 **loaded tools** 实际验收，不是仅stdio：Mac Agent PID52174、四项权限true；专用虚拟屏窗口PID17623/CG12984，独立 attach 不替换desktop绑定。explicit true成功保留elapsed_ms=322/stage rows；默认第二次短回复无timings，details完整保留174ms及terminal/资源释放，AX+receiver计数0→1→2。
- 操作暂停后旧关闭frame过期，真实checked_proposal_expired、0native，未重发；fresh attach+snapshot+新request关闭成功78ms，state.closed/进程0/随后detach。与调色板missing或图像错误不同，这个是本轮操作方复用了>120s旧提案，门槛未放宽。
- **继续实现了F05**：共享 Intent::MovePointer + typed computer.hover（element/view，element不允许action/AXPress，无dwell），原resolve/lease/act/release/no-replay；native单次MouseMoved、Private source、flags/pressure/clickCount/delta0，history unknown，不读写Moved eventNumber，不请求激活/warp/globalfallback。独立原生guard/权限/身份/lease/cancel保留；Windows明确unsupported。
- 输出hover_effect=unverified，独立区分native motion、tracking enter/exit、tooltip text请求与真正显示。Native capability与parser feature分开，不能把implemented当任意窗口hover可用。
- AppKit自有receiver增加公开tooltip owner回调请求计数/point。addToolTip API动态取文本，不是可见性证明：[Apple addToolTip](https://developer.apple.com/documentation/appkit/nsview/addtooltip%28_%3Aowner%3Auserdata%3A%29?language=objc)。测试14→16→rollback14→reapply16（均0），本轮自有timer运行在exec前/后实际采样SHA e179a77e…，不沿用旧PID的binary身份。
- 最终source gates（生命周期源冻结后再跑）：fmt、workspace **631passed/10ignored/41suites**、all-target/all-feature locked offline Clippy -D warnings、双release build均0；locked deps不修改。新Agent真实沿用稳定证书/同DR部署，input lease锁下全部durable状态核查，配置/profile/public CA/证书不变，无recover/clear fence/新权限。
- 新Agent PID66480/instance396fdd62-5f09-455a-b5c8-fa6e67835d81，经loaded MCP真实回查四权限true、pointer_move/parser fields且未选窗。新MCP **39tools/computer.hover** candidate与installed stdio/strict signature0，安装SHA ab74d313fe832ecdec3f35e52ccadd8866456af487339997a201ffbdf8436f67。
- 当前会话工具表仍是重启后那批38工具；F05新工具的 **Host-native motion/tracking/tooltip验收未完成**，需下一次Host重载。没有用新候选stdio的输入替代Host验收、没有把CG构造5tests当实际hover成功。

## CUA/RWI 真实对照试点（不是公平benchmark）

1. CUA getApp('receiver_fixture')返回Invalid app；不据此泛化CUA不支持非标准Mac app。复核公开入口与Apple按bundle identifier解析app的API：[NSWorkspace](https://developer.apple.com/documentation/appkit/nsworkspace/urlforapplication%28withbundleidentifier%3A%29?changes=_____8)。把本地Rust RX-only fixture封装为work中的.app（不global安装、不申请权限）后getApp精确完整路径成功。第一次JS绑定变量未定义错误单列，let新绑定修正；都不是native点击失败。
2. 唯一已授权窗口PID53515/CG13128/虚拟display4：CUA indexed click使counter0→1，接收端OS前台为fixture；wrapper click194.01ms、click+AX read732.54ms、AX diff795 chars。这不是wire bytes/token数或native CPU。
3. RWI对同一按钮explicit AXPress1→2：adapter elapsed271ms（act_checked RPC-stage266ms），tool await287ms、action+snapshot514ms。counter初态/route/focus/readback不同、只一个顺序样本，**不能作速度/成功率优势**。
4. 当前公开CUA接口没有move/hover方法；没有用press/scroll/孤立drag伪造无按钮hover，也没有断言CUA整体不支持。
5. CUA观察后的Window-sharing chrome替代traffic lights，RWI fresh关闭控件不再存在。没有猜sharing按钮；仅核对这个disposable进程的uid/完整argv后SIGINT，进程exit130、fresh exact-PID discovery0、detach。model最后仍running，**不声称state.closed**。这个清理问题促成fixture自有有界lifetime改进，独立于产品input backend。

## 自有测试窗口清理修复及实机复验

- 新fixture默认120000ms，显式 `--lifetime-ms` 限1..600000；无参数/help不会创建窗口或timer。timer只请求关闭被保留的唯一自有窗口；真实windowWillClose才写closed。若按钮动作被sharing禁用，只关闭同一个自有NSWindow，不操作系统sharing controls；自身ApplicationDefined wake只退出自身run loop，不记录鼠标/键盘/hover成功。
- 同一模型命令：BASELINE16→MODIFIED19→主ROLLBACK.sh --lifetime-only恢复16→REAPPLIED19，全exit0，4文件before/reapplied哈希一致。root源没有回退；早期Result类型编译错误/RTK绝对路径包装错误及改动后的成功命令均保留，GUI不根据它们猜测结论。
- 实际虚拟display4新fixture PID89957/CG13354，exec前后binary SHA `e179a77eeec4fd2d57e9a38ed4832c387704d5a6c7a3f547cae9d61dd1bc7ab5`。设置1800ms，prepared→running→lifetime_timer_close_requested→window_closed，status.closed/进程exit0/stderr空；总3597ms含启动，不是硬实时timer benchmark。
- 独立fresh exact-PID discovery匹配0、terminal/资源释放/input=false；process_identity_unknown符合进程已退出，但不是隐藏/最小化窗口的普遍absence证明。safe_count、motion/tracking/tooltip、drag各项全部0，无operator输入。fixture启动观察到app_frontmost=true：这次只验自身生命周期，不证明后台输入、用户焦点不变或hover成功。
- 生命周期修改仅examples，不再次部署Agent/MCP或改TCC。冻结后全仓fmt、workspace631passed/10ignored/41suites、all-target/all-features locked offline Clippy -D warnings、git diff --check实际均0；fixture19tests为独立范围，不凑入631。

## 实际截图（未编辑，按顺序）

重启后的counter1

![重启后的counter1](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-08/post-reload-and-hover-20261008/timingSnapshot1.1.png>)

同窗CUA/RWI试点counter2与sharing chrome

![同窗CUA/RWI试点counter2与sharing chrome](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-08/post-reload-and-hover-20261008/comparisonSnapshotAfter.1.png>)

## 后续完整计划仍保留

下一验收先以fresh target在专用窗口跑computer.hover，分别读moved/tracking/tooltip oracle；失败定位到guard/投递/客户端，而不是再退成“hover不可能”。继而是fully-occluded/offscreen/other-Space/minimized/hidden、AppKit/Electron/Wine/SDL/GLFW同状态可逆对照、cold/warm/P50/P95与native阶段拆解、并发/回收/Live/多窗/模型输出。工具表新增确实需要本次Host重载；后续应批量部署并评估安全的专属MCP重载，不强杀Codex或其它会话。

原四角色沿用同10/06事务；新17源路径成组rollback防止共享enum/原生match不一致。证据、命令数组、literal streams/exit、实际SDK错误和修正保留在：
/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-08
