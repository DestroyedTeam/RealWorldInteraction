# Paint S Open 面板 AX 命中诊断

通过真实Plugin观察已绑定PID2578、cgwindow5573、bounds[316,283,880,448]。只读诊断最多保留8条，存放adapter_status.ax_hit_diagnostics，精简摘要不展开，不读取控件文本值。

| 控件 | 节点AX窗口 | 节点中心 | 原生命中 |
|---|---|---|---|
| Cancel AXButton |5575|1049.5,699|AXSheet 5573 / [316,283,880,448]|
| Open AXButton |5575|1136.5,699|AXSheet 5573 / [316,283,880,448]|
| 搜索AXTextField |5575|1079,314.75|AXSheet 5573 / [316,283,880,448]|

双方AX PID均2578，CFEqual=false。所有on-screen同用户目录分页中没有5575；因此不能将它凭猜测绑定为另一个实际CG窗口。坐标落在面板内，但应用级命中只返回外层壳。

额外只读实验将窗口对象传给hit-test，所有测试点返回不可用；没有改为授权成功。已从最终实现中移除这一额外探测，以免每次观察增加无效调用。Apple接口文档定义application或system-wide参数，最终实现保持该契约。

这说明当前原生指针命中不能证明这些远程面板控件；尚不能据此断言Paint S不接受输入或可完全后台操作。候选方向为区分精确AX语义动作和屏幕指针动作：通过新鲜绑定子树重新定位唯一原生对象、核对动作/状态与父链，语义操作不伪造屏幕hit命中；涉及核心解析/输入边界，需要单独实现与负向测试。当前未放宽可见性门禁，未发出面板输入。

同轮重新检查全部3张参考附件，实际目标是attach-3-bkkRer32.png。该图第7行有完整头像、第8行有部分头像，旧接续记录无头像说法已修正。

参考：[Apple AXUIElementCopyElementAtPosition](https://developer.apple.com/documentation/applicationservices/1462077-axuielementcopyelementatposition)。

## 精确原生语义动作实现与现场验证

后端显式公布支持的AXPress/AXOpen/AXConfirm。act_checked仅在Click+明确动作、后端支持且控件公布该动作时签发service-only native_action引用；普通resolve/pixel/点击不获得此权限。语义引用不能改发其他动作、鼠标、文本。原生层在绑定窗口内按观察ID重新遍历唯一对象，重复/缺失/隐藏子树/深度节点队列时间超限拒绝；沿用过程会话、框架状态revision、窗口范围、非安全控件、模态、lease与durable no-replay检查。不用点命中代替原生对象，也不执行前置primer点击。

新增核心测试覆盖后端不支持、禁用、revision变化、未指定动作、未公布动作、重复请求去重及禁止转为指针动作。439 Rust tests passed /10 ignored，fmt、全目标全特性clippy通过。

通过Plugin对原Open面板发出`v14-semantic-cancel-20260919-01`，AXPress returned，1361.6ms，随后独立discover确认Open列表为空。再以`v14-semantic-addimage-20260919-01`点击Add Image，818.0ms，独立discover/observe确认新面板及控件出现。两次均没有fallback或重放。

当前已跨过“面板按钮无法操作”的阻塞，但文件导航、头像导入/定位和最终图层验收尚未完成。画布无新增图像。

## 导航到Home后的跨进程证据

`v14-panel-home-20260919-01` 对侧栏Home单次AXOpen返回-25205；独立截图和Where值确认已经到vincentadamnemessis主目录，没有重放，查询终态并恢复输入域。

AI Projects图标进一步触发semantic_target_ambiguous且native_dispatch_entered=false。将错误分成原生对象冲突和进程不匹配，并允许CFEqual为真的同一对象重复出现（不同对象仍拒绝）；固定签名部署后，新鲜观察下新请求`v14-projects-after-identity-fix-20260919-01`精确返回semantic_target_process_mismatch：控件PID46633，绑定PID2578；native_dispatch_entered=false。独立ps验证46633的UID501，路径为/System/Library/Frameworks/AppKit.framework/Versions/C/XPCServices/com.apple.appkit.xpc.openAndSavePanelService.xpc/Contents/MacOS/com.apple.appkit.xpc.openAndSavePanelService。旧请求均不可重放，已查询终态并恢复输入域。

结论：按钮和侧栏部分节点由宿主代理，实际文件图标由系统XPC进程提供。当前停在Home，尚未导入头像。后续委托归属验证须保留绑定子树来源、同用户/进程出生、系统服务身份和原生对象检查，并验证生命周期变化；不能只凭不同PID或服务名字放行，也不能按全局文件名搜索另一个进程对象。

本轮439测试通过，fmt/clippy通过。

## 已实现系统托管provider证明

原生Rust调用SecCodeCopyGuestWithAttributes(PID)和SecCodeCheckValidity，requirement为`anchor apple and identifier "com.apple.appkit.xpc.openAndSavePanelService"`。前提仍是候选对象从当前精确窗口AXChildren树找到，且进程属于同UID；元素ID绑定provider进程出生，提交前再检查。单凭路径、名字或“是系统进程”均不授权。当前provider实现不声称覆盖字体/颜色/其他第三方窗口。

现场：`v14-signed-delegate-projects-20260919-01`进入AI Projects；`v14-delegate-repo-20260919-01`进入Real World Interaction。两次AXOpen返回-25205，但后续Where原生值和项目目录截图证明导航变化；无重放，分别查询原操作并恢复输入域。当前work图标可见，但元数据输出在128原生节点预算截断，后续需补大目录支持。

参考：[SecCodeCopyGuestWithAttributes](https://developer.apple.com/documentation/security/seccodecopyguestwithattributes(_:_:_:_:))、[SecCodeCheckValidity](https://developer.apple.com/documentation/security/seccodecheckvalidity(_:_:_:))。
