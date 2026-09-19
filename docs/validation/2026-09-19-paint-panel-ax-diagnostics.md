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
