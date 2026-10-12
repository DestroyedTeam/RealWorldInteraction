# R65：真实系统文件面板与模态输入恢复（2026-10-12）

## 范围与实现

专用Rust/AppKit fixture 的显式 `--system-panel-controls` 增加 Files 按钮：真正调用公开NSOpenPanel／beginSheetModalForWindow，初始位置为该fixture状态目录中新建的空目录。只取消、不选择或打开用户文件。`SystemPanels.file` 分开记录请求次数、原生window/sheet/父窗口/显示器/CGWindow读回和真实completion response；响应0不等同窗口已消失。完成队列有上限及丢弃计数，不在可重入框架调用中借用UI状态。

这是验收夹具，不是生产隐藏桥接；GUI操作均由本任务当前已加载RWI MCP发出。生产Agent仍是R64 PID34348／cc9272d1，未再部署或改证书／配对／权限。

## 真实顺序与问题

1. **148**（PID89003／parentCG44616）在虚拟屏4。Files AXPress一次返回uncertain／AXError-25204；随后只读发现真实文件sheetCG44629，open_actions1，is_sheet与attached_to_own_parent均true，CG owner为fixture PID，完整880×448点位于虚拟屏4。父窗口由3036移到2950是AppKit布局实际变化，未拿旧几何继续输入。没有重发Open，也未把uncertain改写为returned。
2. 对148父窗口Safe执行明确模态负例：checked_context_unavailable，native_dispatch_entered=false，Safe仍0。对真正sheet的Cancel AXPress一次returned；原生completion_response0／visiblefalse／parent关联解除。完成callback与呈现消失分别验证，不靠枚举onscreen缺失键推断false。
3. 148后来在480秒期限自然退出0／expiry1。晚到的discover为空、snapshot无control之后停止；Safe-after-cancel和Close根本未发送，不冒充显式关闭。两个binding已detach。
4. **149**是新的独立测试进程（PID2328／parentCG44674），不是148输入重放。其Open同样uncertain；首次有界只读发现未观察到sheet，不能认定未执行或进程终止。保留超时，重新读同一原进程/句柄和状态后发现实际sheetCG44684，未重新启动、未再次Open。
5. 149真正sheet Cancel一次returned，之后精确PID discovery只剩parentCG44674。fresh parent snapshot blocked=false；Safe AXPress一次独立计数0→1，同时app inactive／nonkey／前台仍97732。无文件打开或保存，完成响应0，队列drop0。
6. 149父窗口明确AXPress Close一次returned，进程exit0、expiry0；两个binding已detach。

## 原始截图（真实先后，不重绘）

![149源窗口](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r65-native-system-file-panel-2026-10-12/01-own149-before-native-dialog.png)

![149真实NSOpenPanel](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r65-native-system-file-panel-2026-10-12/02-own149-native-file-sheet.png)

![取消后后台Safe1](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r65-native-system-file-panel-2026-10-12/03-own149-cancelled-and-background-safe1.png)

## 验证与保留错误

- 7个原请求durable回读全部terminal/released、无pending/cleanup uncertainty，replay_allowed=false；两个Open的uncertain和原AX-25204保留。界面稍后变化不是重放的理由。
- 初次编译exit101因为appkit直接调用私有State::record；改为窄的native_file_panel_completed方法及测试，不把record广泛公开。两项新增fixture测试实际运行，完成响应不推断window消失或所有者。
- 完整11门exit0：workspace774／0 failed／11 ignored，Receiver72／Live UI7，fmt、locked/offline tests、all-target/all-feature Clippy-Dwarnings、双release、架构门禁均真实执行。
- 同四角色配置BASELINE1／MODIFIED0／ROLLBACK1／REAPPLIED0，独立源副本恢复原始哈希；不是生产回滚或输入重放。

## 结论与未完成项

原生系统文件面板、父窗口模态拒绝、正确sheet取消与父窗口后台操作恢复已接受。此机器上渲染owner仍为fixture PID，不能冒充foreign-PID支持。快照中部分control被标occluded，单独SCK窗口图像不证明物理遮挡状态；本轮只使用明确AXPress，不放宽point/visibility守卫。

接下来复用已有LiveFamilyCapture，实际检查根窗口与真实NSOpenPanel的已验证AXSheet/AXParent层叠合成。不能用plain own-sheet、resource URI或帧计数替代这一新OS面板案例。通用冷态hover、跨PID/provider图、隐藏/最小化/其他Space、原生DnD、浏览器独立route及完整压力/耗时矩阵仍保留，整体验收未完成。

API依据：[Apple NSOpenPanel](https://developer.apple.com/documentation/appkit/nsopenpanel)、[sheet完成接口](https://developer.apple.com/documentation/appkit/nssavepanel/beginsheetmodal(for:completionhandler:))；本机SDK原始声明已保存，明确completion可早于实际画面退出，效果以上述本机读回为准。
