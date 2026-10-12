# R64：切窗后的引用失效反馈（2026-10-12）

## 修改与结论

`Engine::focus_window` 在真正执行同桌面alias失效后，额外返回紧凑的 `grounding_invalidated`：`context=current_executor`、`scope=same_native_desktop`、`target_count`、`next=snapshot_then_resolve`。这是当前执行器的目标范围，不是整个Host所有聊天；目标数量也不是被删除的frame数量。未删除安全校验、自动刷新、激活回退或输入重放。

核心合成fixture覆盖返回、零派发拒绝、uncertain、未知counter；持久重复读取不宣称发生新失效。新增断言先实际失败exit101，修改后同一命令exit0；完整11门exit0，workspace774/0/11、Receiver70、Live UI7。

## 实机顺序

全部GUI输入来自本任务已加载RWI MCP，只操作Virtual4上的专用146／147。新Agent34348／cc9272d1-0300-4ae6-9b1d-1f1ab72f7d2b以既有稳定证书部署，四权限有效，配置／CA／profile／已安装MCP不变，不需Codex重启或人工配合。

1. 对独立绑定146／147各取snapshot；明确focus147一次。真实前台回读为PID34877／CG44482；反馈实际 `target_count=3`（本执行器的桌面目标和两个固定窗口）。独立AppKit记录147active/key、146inactive/nonkey。
2. 只读解析146旧frame得到 `unknown_frame`／`native_execution=false`，与返回的失效范围一致。没有拿缺失reference去调用act，也没有额外输入。
3. 重新snapshot146，再对其“Safe count: 0”执行一次明确AXPress。独立状态为Safe1／sequence1，仍inactive/nonkey／前台34877；新snapshot读到“Safe count: 1”。这是后台原生语义动作，不是鼠标点击证明。
4. 分别明确AXPress Close147／Close146；两进程exit0、expiry请求0，两个绑定detach。四个原请求durable回读全部terminal／released，无pending、cleanup uncertainty或replay。

## 原始截图（实际时间顺序）

![切窗前源窗口](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r64-focus-grounding-feedback-2026-10-12/01-source-before-focus.png)

![切窗后重新观察并后台AXPress](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r64-focus-grounding-feedback-2026-10-12/02-fresh-background-axpress-after-focus.png)

## 验证、范围与后续

- 同四角色继续扩展；matching源配置BASELINE1／MODIFIED0／ROLLBACK1／REAPPLIED0，独立副本的原始哈希精确恢复；不是生产Agent回滚或输入重放。
- 保留一次apply_patch目标行经fmt换行而不匹配的工具错误（没有shell退出码，不杜撰数字），调整为实际代码site后成功。额外探查误写不存在output.rs的rg exit2／Python exit1也保留，后续改读实际声明，不把它当权限错误。
- R63的默认／实验后台Move与前台tracking证据保留，没有重新投递。通用冷态后台hover、foreign-provider系统弹窗、隐藏／最小化／其他Space、原生跨app DnD及browser内容route仍不能以本测试的成功替代。
- 当前更改未推送；远程已确认通过的仍是R63 `f2204d02`／CI38209937792（macOS与Windows success）。将在相关功能组验收后合理拆分提交，不为了每轮聊天重复触发CI。

本轮是工具反馈修复及其实际效果验收，不是整体验收完成。下一步优先补原生系统文件面板／提供者窗口层级的真实案例；人工保持不参与。
