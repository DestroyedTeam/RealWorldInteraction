# Live 小型预览：紧凑绘制与真实按钮区域（2026-10-13）

## 已完成源码变更

保留已部署的小型预览上限280×300pt、48pt顶部区域及现有隐藏／恢复／关闭语义。只修改 RWI 自有原生预览控件；不要求人工参与，也不改用户应用。

隐藏／关闭按钮的源码 NSButton frame 从28×28改为44×44pt，两框间隔4pt、顶部留2pt；专用 NSButtonCell 的绘制几何保持28×28。缩短标题／源说明可用列，避免与按钮重叠；图片和极短图占位说明均不侵入控件区域。

使用自有 RWILiveAccessibilityButton 的 +cellClass getter，不全局替换 NSButton、不 swizzle、不伪造 hitTest。正常／interior／bezel／highlight／focus绘制转发的 compact_frame 幂等处理28pt；target/action/tag、AXPress、enabled 与原生 trackMouse 全部保留。

依据：[Apple：Subclassing NSButton](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Button/Tasks/SubclassingNSButton.html)、[Apple：NSControl](https://developer.apple.com/documentation/appkit/nscontrol?language=objc)、[Apple：Buttons](https://developer.apple.com/design/human-interface-guidelines/buttons)。这是基于公开 API 的 RWI 实现，不宣称复制了 Codex 私有叠层的内部代码或精确像素样式。

## 实际观察：源配置而非原生点击

同输入280／269，分别冻结并编译真实旧 constructor 表达式、新 preview_layout／compact_frame 函数，使用仅标准 Rust NSRect 数据代理：
- BASELINE 编译0；按钮框 [204,235,28,28]／[240,235,28,28]，paint helper 不存在标为 null／unknown，未满足44pt规则，配置检查exit1。
- MODIFIED 编译0；按钮框 [180,223,44,44]／[228,223,44,44]，paint数据 [8,8,28,28]，配置检查exit0。
- 5项新的真实 Rust 几何测试通过。
- ROOT fmt、workspace test、browser example test/build、Receiver example test、全target／feature Clippy -Dwarnings、架构检查／单测及git diff --check，9条实际命令均exit0。
- 原373项源码扩展为374项，全部实际哈希冻结。生产 Agent、MCP、签名证书／配对／权限尚未改变。

这些结果不证明 AppKit 实际绘制、外围点击跟踪、AX44pt回读或动态 initializer ABI 已验收。没有新原生画面就不添加伪造的截图。

## 同角色回滚及下一项

旧三源文件及新 compact_cell.rs 的 ABSENT 已在修改前记录。后续用同一 ROLLBACK.sh 的 --live44-compact-controls-only 在独立标记副本恢复三源／删除helper，匹配输入继续编译／检查，并重应用；完整原256基线及新增helper／两报告ABSENT均不重基。实际命令和结果由以下同角色与proof记录保存，不把准备状态当完成：

- [源码候选实际结果](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/live-44pt-hit-region-readonly-2026-10-13/parseable-proposal/candidate-transaction-live44/RESULT.json)
- [九项实际门禁](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/live-44pt-hit-region-readonly-2026-10-13/parseable-proposal/candidate-transaction-live44/OFFLINE_FINAL_GATES_LIVE44.json)
- [源码374冻结](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/live-44pt-hit-region-readonly-2026-10-13/parseable-proposal/candidate-transaction-live44/MODIFIED_SOURCE_FROZEN_LIVE44.json)
- [同角色执行器](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/live-44pt-hit-region-readonly-2026-10-13/refresh-four-roles-live44.py)
- [MODIFIED_FILE](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/MODIFIED_FILE)
- [DIFF_FILE.patch](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/DIFF_FILE.patch)
- [VERIFICATION.txt](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/VERIFICATION.txt)
- [ROLLBACK.sh](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/ROLLBACK.sh)

部署仍待同角色回滚验证完成后，使用原稳定本地证书及同App路径构建／签名／替换 Agent；MCP不需要为了这次UI变更替换。若运行器发现忙或签名／权限问题，记录真实失败，不伪成功、不要求睡眠中的用户授权。

## 待实机自主验收

分配专用源窗口与虚拟预览显示，不操作用户游戏／Chrome／文档。两个按钮分别测试44pt框外围8点、4pt gap、框外点、按下后在框外松开、disabled拒绝；独立核AX frame／标签／窗口身份。再核正常／hover／pressed／focus紧凑绘制、隐藏清图、恢复新帧、关闭停止采集且源窗口存活。RWI前置拒绝与cell执行效果分别记录；用户亲眼可见及真实物理人工输入不由模拟测试代替。


## 后续实际部署完成（同日）
前述部署待执行是候选生成时的状态。其后同四角色259的默认与source-data配置四相实际均1／0／1／0，scope四相编译均0；源和原基线哈希／ABSENT恢复及重应用已验证。
五条实际release build／复制／稳定签名／DR验证命令均0；原R98部署事务实际0、旧服务65656实际退出、原子替换、启动及post-smoke全部0。新Agent PID61970，instance e52390d8-8c73-472a-a79e-0b8f527aa9c7，签名程序SHA a54267d4b0602d45e9d9db206ddd071cc6a19c12a1075bd38361a07326184f5b。原cert24B346及DR保持；仅安装的Agent程序变化，受保护的其他11公共文件相同。
已加载MCP实际重连读到accessibility/listen_event/post_event/screen_capture四项true，requests_permission=false、input_domain_fenced=false、resource_domain保持。没有重启Codex／MCP、没有创建／导出证书密钥或更改配对、没有人工授权。
部署与传输验收不等于Native44外观／AX／鼠标外围命中验收；这些仍待专用源窗口。
[真实部署事件](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/live-44pt-hit-region-readonly-2026-10-13/ACTUAL_AGENT_DEPLOY_DISPATCH_LIVE44.command_execution.json)
[已加载工具新Agent实读](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/live-44pt-hit-region-readonly-2026-10-13/ACTUAL_LOADED_TARGETS_AFTER_DEPLOY_LIVE44.json)


## 后续真实原生验收（有限范围，已执行）

部署后的自有Live面板位于虚拟屏4，实际窗口280×269点，原PNG560×538。两个绘制圆区RGB差阈值4量测56×56px（约28pt）；阈值8/12的55px抗锯齿边缘不冒充AX框。源码声明的44pt控件框与28pt视觉分开：8个隐藏按钮外围/角点（在声明44框内、28视觉外）均实际隐藏、清图、离屏，API恢复后重新运行/显示；4pt间隙一点和按下后移出松开一次未触发动作；关闭按钮外围角点一次停止并清图，源测试窗口仍存活。

73个原始工具记录中12个唯一mutation均terminal/released，无pending uncertainty、无重放。源81362生命周期实际exit0，两个自有源已清理；不操作用户游戏、文档或要求人工配合。原回执goal_verified=false保持，效果来自独立后续状态。13张原PNG已核hash（11张稳定panel图相同hash，不充作不同视觉变化）。

**范围边界：**没有读到实际AXPosition/AXSize的44pt矩形，不能声称AX44已实测；只验证关闭按钮一个外围点，不扩充为八点全验。没有重验物理内建屏/其他缩放/暗色高对比，没把本次当任务切换的新证明，更不声称Codex私有UI像素1:1。R73已验小窗/控件语义与历史任务绑定结果保留，不重新要求休息用户配合。

原始画面（Root将按hash原样复制，无图像编辑；PNG不入文本diff map）：

![已部署紧凑Live按钮](/Volumes/ChatGPT/Real World Interaction/outputs/validation/live44-compact-native-controls-2026-10-13/01-native-compact-controls.png)

![关闭预览后源窗口仍存活](/Volumes/ChatGPT/Real World Interaction/outputs/validation/live44-compact-native-controls-2026-10-13/02-source-survives-close.png)

独立有限审计：[/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/live-44pt-hit-region-readonly-2026-10-13/native-source-lifecycle-live44/READ_ONLY_FINITE_NATIVE_PIXEL_AND_HIT_AUDIT_LIVE44.json](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/live-44pt-hit-region-readonly-2026-10-13/native-source-lifecycle-live44/READ_ONLY_FINITE_NATIVE_PIXEL_AND_HIT_AUDIT_LIVE44.json)；原工具ledger与所有错误保留同目录。Chrome非活跃接收仍未解决，全F01–F16未封版。
