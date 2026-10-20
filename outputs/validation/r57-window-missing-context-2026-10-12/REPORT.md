# R57：窗口未呈现不等于窗口已销毁

日期：2026-10-12（Asia/Shanghai）。只操作已授权的自有测试窗口，未请求人工配合。

## 本轮结果

| 项目 | 实际结果 |
|---|---|
| 旧版同一 Colors 窗口 | snapshot、capture_visual 均报 window_missing，无进一步原生诊断 |
| 新版同一 PID29881 / CG43059 | 两路径仍拒绝截图，但精确 WindowServer 记录存在、owner匹配 |
| 可选 on-screen 字段 | null；没有解释成 false，也未猜测 destroyed/hidden |
| 输入、回放、画面授权 | 无图像、frame、view、controls、mapping；native_dispatch_entered=false |
| 新 Agent | PID34800 / instance84129a56-ab4a-4672-91dc-0b18ef867255；稳定签名，四权限有效 |
| 生产配置 | 配对/CA/profile/MCP字节不变；无需重启Codex或重新授权 |
| 当前回归 | 11门exit0；workspace760通过/0失败/11忽略/42套；Receiver64、Live UI7 |
| 清理 | 两source各一次fresh AXClose；closed、expiry0、进程exit0，六binding退役 |

`NativeMac::window_record` 与 `NativeMac::visual_window` 在 on-screen 查找失败后，调用只读 `window_record_probe::read`。该公开API只查询已绑定的一枚windowID，不查询外部窗口标题，不授予输入，不自动激活、解除隐藏或重放旧动作。存在记录不代表能截图或输入。

第一次打开Colors时AXPress返回-25204/uncertain；立即oracle计数0，后来独立读回计数1、CG43059出现。原始不确定回执仍保留，未重发Colors。七个原durable请求均终态、资源已释放、无pending、无release uncertainty、no replay。

重新绑定Colors后，切回Codex造成它再次退出on-screen列表；此时新版正确返回诊断，而不是要求再次点击。这是非原子状态变化的实测，不能将一次focus成功解读为持续前台保持。

## 实际时间顺序原图

1. 自有源窗口，Colors打开前。

![自有源](01-own-source-before-color.png)

2. 原Colors窗口已搬到Virtual4；原532×908，不改变图片字节。

![真实颜色面板](02-own-color-on-virtual-display.png)

3. 同一源窗口最后一次独立观察，之后仅执行一次关闭。

![源关闭前](03-same-source-before-explicit-close.png)

隐藏时两次截图都是错误，**没有生成假截图或沿用上一帧充作新画面**。

## 配置回滚验证与原始错误

相同源配置检查：BASELINE false/exit1 → MODIFIED true/exit0 → 独立副本执行ROLLBACK false/exit1 → REAPPLIED true/exit0。三个旧文件hash与新模块不存在状态精确恢复。它不是生产Agent回滚或隐藏窗口输入验收。

新的source-wiring回归先失败101、后通过0；四个projection测试通过。首次完整门禁Clippy因len_zero失败101，已改用is_empty并以完整11门复验通过；不隐藏失败。

## Live尺寸与参考

既有Live继续采用最多320pt宽、按源比例紧凑呈现；eye.slash隐藏、play.fill继续、xmark关闭，有原生提示与AX名称，无经典三按钮。R48已真实验过Host v2任务切走隐藏/清图/停采、返回同binding恢复、关闭停止而源窗口存活；本轮保留该结果，没有伪造组件presence。

官方Computer Use页面说明macOS画中画可移开，但未给出精确像素/按钮样式，本项目不声称私有UI的1:1复制：[OpenAI官方说明](https://learn.chatgpt.com/use-cases/use-your-computer-with-codex)。Apple浮动panel应小而少遮挡：[isFloatingPanel](https://developer.apple.com/documentation/appkit/nspanel/isfloatingpanel)。

窗口隐藏策略和只读描述参考：[hidesOnDeactivate](https://developer.apple.com/documentation/appkit/nswindow/hidesondeactivate)、[Quartz Window Services](https://developer.apple.com/documentation/coregraphics/quartz-window-services)、[可选窗口字段](https://developer.apple.com/documentation/coregraphics/optional-window-list-keys)。

## 未完成范围

这里只补诊断，不代表通用后台hover/tooltip、minimized/offscreen新鲜像素、foreign-provider共享菜单、任意隐藏窗口输入或跨应用DnD完成。F01–F16完整计划继续；用户休息期间不请求切窗、展开、密码或授权。
