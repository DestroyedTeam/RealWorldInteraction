# 2026-10-09：更新后游戏像素拒绝与构造理由修复

## 结论与范围

TARGET始终为 `/Users/vincentadamnemessis/AI Projects/Real World Interaction`。本轮只读原始执行记录、修改/验证Rust候选，**未发现、捕获、输入、激活或重启游戏，也未部署/重启Agent**。导航会话继续拥有游戏控制权。

原始request `navigation-post-update-city-minimap-lower-interior-20261009-1` 的[完整持久回执](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/anchor-creation-reason/CITY_ORIGINAL_DURABLE_RECEIPT.json>)确认：
- Agent `de7cc66c-8a96-4730-8f4d-a569b711e483`、原context `bf8a6d36-ad65-4d11-9f79-bceb08b0ac51`，结果terminal、resources released、无uncertainty、replay forbidden。
- 顶层stale_reference，旧generic message；reference_kind=pixel；content_changed=true，scope/focus/layout/nativeEpoch/mutationcontext/mutationready/blocked changed均false。
- **local/marker诊断对象完全缺失**，不是“已有对象但数值metrics=null”；native_input_dispatched/native_dispatch_entered均false。
- 按部署前Core的evaluate_grounding分支，这个形状定位到**无保留局部/marker锚点、全帧内容不相等**。这是代码与回执的联合推断，不是历史PNG位置差异测量。
- 回执不含原构造失败理由、原PNG与重验PNG，所以**不能确认是小地图外无关动画造成“误拒绝”**，也不能归因空AXChildren、低纹理、权限或游戏地图错误。
- 上报2501ms/act2406ms是源会话提供的adapter墙钟；本轮未重测。它包括transport/queue/revalidation等，不证明PNG/纹理CPU耗时。没有使用另一adapter的短期details_ref或当前last_observe覆盖态补造历史。

## 对症源码修复（不是放宽输入）

1. `PixelAnchor::from_png_diagnosed` 与 `features_diagnosed` 从**原始一次构造**返回typed `CreationFailure`。
   区分原有尺寸/边距/有限值/字节及像素预算、header/frame解码、dimensions/buffer/格式/长度、`luma_span_below_minimum`等实际失败分支。
   旧Option行为用test-only `.ok()`对照；没有第二次解码来猜理由。
2. `Engine::resolve_action` 记录backend未opt-in、原artifact缺失/淘汰、实际构造failure。semantic/window/red/success不被标成普通pixel失败。
3. 私有 `StoredReference` 将理由与原引用放在**同一个有界VecDeque**，容量仍256，随原retain/clear/evict退休；没有额外容易泄漏的理由map。公开 `Resolved` 与MCP输入schema没有新增字段。
4. `Engine::act_inner` 顶层拒绝仅取已存固定reason，写入 `local_anchor_creation_reason` 和短错误文案。
   例如离线uniform fixture实际显示 `luma_span_below_minimum`；**这不是这次真实游戏原因**。
   原未知理由仍 `not_recorded`。Native first-input-batch层仍使用原public Resolved，本次不冒称那个阶段也新增同样理由。

**没有改变**96px patch/64px边距、luma32、32/384/16匹配预算、解码限制、feature计算、full-frame equality、native surface/identity/focus/modal校验、lease/reservation/no-replay或input authority。没有新增desktop有限值门槛；原行为用对照验证。
低feature contrast是6×6 cell平均后的跨度，不等于视觉上完全无纹理。

## 当前不降级的调用办法

- 由拥有游戏控制权的会话fresh snapshot/fresh resolve。若原artifact恢复且原点满足原有局部纹理条件，同一点可能获得正常local anchor；若完整frame确实不变，原full-frame规则也可通过。两种均**不是保证成功或自动重试原request**。
- 如原点不符合条件，只能在**同一业务目标的合法点击面**选择经fresh图像验证的稳定纹理点；这不是把换点成功算作旧点修复。
- 显式 `red_marker_cluster=true` 只适用于实际看见并命中合格compact red cluster的目标；不能套到普通小地图底部或借它忽略任意动画。
- empty AX children与mutation_ready=true不保证anchor存在；局部anchor还要求保留原artifact和构造成功，随后native仍需精确允许canvas证明。
- 不能强制接受旧ref、屏蔽contentChanged、降低阈值、借其他输入通道、伪造AX控件或重放原request。

## 实际验证与三态

| 检查 | 观察结果 |
|---|---|
| Pixel构造namespace，同命令 | BASELINE14pass → MODIFIED19pass，均0；实际failure与原Option结果一致，成功同一次feature extraction |
| Core新断言，同一uniform→远端变化输入 | BASELINE101：旧not_recorded不满足精确reason断言 |
| MODIFIED | 4tests通过/0；正确保存luma/artifact/backend/legacy/有界退休；仍full_frame_equality、0dispatch、无core receipts |
| 主ROLLBACK.sh三文件联动独立副本 | 原三hash在加test-only harness前逐项相等；同断言再次101/not_recorded |
| REAPPLIED | 同namespace4pass/0；根源码始终修改态，未回滚生产 |
| 完整冻结门禁 | **653pass/11ignored/41suites**；fmt、locked/offline workspace tests、all-targets/all-features Clippy -D warnings、diff check均0 |
| Agent release | build0；unsigned SHA `6b188d7dbfb4c7434531b4081fa23276cd0b7c86e7c38e2a60aa059aa107e201`，未签名/安装/执行 |

[主三态与hash](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/anchor-creation-reason/MAIN_ROLLBACK_RESULT.json>)、[Pixel冻结](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/anchor-creation-reason/FROZEN_SOURCE.json>)、
[Core汇总](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/anchor-creation-reason/core-retention/SUMMARY.json>)、[完整门禁计数](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/anchor-creation-reason/FINAL_COUNTS.json>)保留。
首次peer测试helper的usize/u64编译错误101是编译错误，不作预期语义红；原stderr与流引用修正也保留。没有删旧结果或把旧测试换label冒充新验收。

## 部署端与真实验证要求

本轮是Core/Agent内部与错误输出修复，**需要重新构建、同稳定证书签名并部署macOS RWI Agent**；当前实际Agent仍旧de7cc66c/e1f0…，所以旧回执没有新字段是预期版本差异。
本补丁不新增MCP工具/参数，不需要仅为它替换MCP或重启Codex；已验过的Agent重连机制仍要实机核验，不把本次离线gate算成重连通过。

部署会使原Agent instance/contexts/frames/views/Live绑定失效，必须先与导航唯一worker协调停止/释放输入，而不是趁它使用服务时重启。
部署后由原控制会话重新targets(mac-host)→fresh discover实际新PID/window/epoch→attach→snapshot→新request；保持业务checkpoint和旧durable结果，禁止回放旧request。

真实验收至少：
1. **同实际场景/同点**保存原始PNG及当次重验PNG、原mapping/native身份与完整原始diagnostic，不用后来截图替换拒绝时输入。
2. 先读新reason，分别验证artifact/格式/边距/contrast真实分支；判断变化位置是否确在目标外，不能仅凭content_changed下结论。
3. 稳定texture目标正例、目标内变化/移位/遮挡负例、无anchor/full-frame变化0dispatch；操作后的应用效果独立核对。
4. 记录stage timings/负例派发标志/资源释放/no-replay；原受控游戏goal由导航会话验收，不由本报告代判。
这次修复的是**原因可观测性与模型可读性**，不宣称原小地图点已能通过或后台hover已修复。

## 同一四角色（扩展，不替换）

- [MODIFIED_FILE](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/MODIFIED_FILE>)
- [DIFF_FILE.patch](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/DIFF_FILE.patch>)
- [VERIFICATION.txt](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/VERIFICATION.txt>)
- [ROLLBACK.sh](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/ROLLBACK.sh>)

完整计划仍保留Hover真实构造/派发矩阵、Live任务切换/HideResume、Mac activation缺口、多窗口/OS panels、应用适配与性能/资源/输出优化。本新增反馈改变优先级，不把计划缩成一个诊断字段。
