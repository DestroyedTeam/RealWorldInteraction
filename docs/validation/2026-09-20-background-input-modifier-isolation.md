# 后台输入与全局修饰键：修正范围

## 根因

用户在其他应用使用 Alt+Tab，却被 RWI 恢复检查要求等待。真实链路为 AXPress 返回非零 → Agent 把业务效果不确定当作可能遗留按键 → recover_input 检查全局 HID/CombinedSessionState。全局按键状态并不能证明用户在操作 Paint，也不能归属给 RWI。

另外，合成 CGEvent 的 source 使用 HIDSystemState，pointer 发送前没有显式清除修饰键。RWI 已经采用 postToPid，问题不是“没有模拟事件”，而是输入状态和错误恢复的作用域设计不准确。

## 本次修正

- 普通输入和背景 primer 改用 Private event source；pointer 显式设置空 flags；keyboard 保留请求明确指定的 flags。
- AXUIElementPerformAction 非零回执保持 uncertain / no replay，增加 held_input_resources=false 原生路径证据。
- Agent 仅在该精确AX证据存在、native call已完成且资源释放无疑问时，不再设置整个输入域的按键恢复锁。未知来源/字段缺失/真实清理失败仍不豁免。
- 修正旧的离线测试：不再向任意PID调用 deliver_key，改为仅检查内存中的 stamp 字段，不发送输入。
- 回归覆盖：pointer 的模拟 Alt/Command flags 被清除；显式键盘 Command 保留；AX业务不确定不触发global-idle恢复；清理不确定、缺失/伪类型字段、CGEvent来源仍保留门禁。

## 明确未完成

本次不是完整的跨进程输入资源所有权账本。真正中断的CGEvent拖动/按键恢复仍存在保守的全局探测，不能承诺所有情况下都与用户完全并行。不能把“私有事件源”当作所有应用的后台兼容性保证，也没有要求用户释放或向系统补发按键。既有owner/窗口/context/lease/no-replay检查不变。

下一步需独立证明每个RWI输入序列的down/up所有权、失败后的有界清理与跨Agent重启证据，再替换旧的全局恢复探测；在真实目标中验证用户Alt+Tab期间的输入与弹窗路由。

## 依据

[Apple CGEventSourceStateID](https://developer.apple.com/documentation/coregraphics/cgeventsourcestateid)：远程控制程序可使用Private事件状态表，使生成和跟踪的事件状态独立于其他进程。该文档不保证所有目标应用如何处理事件。

## 验证与部署

fmt、workspace offline测试443 passed / 10 ignored、all-targets/all-features clippy通过。固定本地证书重签部署，备份work/v14-modifier-isolation-20260920/previous-bundle；四项系统权限仍true。新实例b3657ab9-28c9-4e5b-84eb-5fa2ed29ca78，真实Plugin重新绑定Paint主文档，执行切换Round Rectangle得到dispatch=returned、fenced=false。本次现场调用没有产生非零AX返回，因此“非零AX不锁全局输入域”目前由离线回归证明，尚未完成用户实际按住Alt+Tab时的端到端对照；不把普通成功调用夸大为该场景验收。

## 观察副作用审计

继续复刻时再次出现普通拖动回执returned但独立截图没有新增图形。检查native.rs发现observe()在截图前调用activate_background_window，后者实际向目标PID发送AppKit激活事件。这与read-only观察契约冲突；也意味着前后截图本身可能改变内部key-window/chrome状态。用户报告的跨屏弹窗是否由此导致仍未证明，不能混为已确认根因。

已从observe移除激活事件与对应等待，只保留原生只读metadata/capture和已有有界重读。显式输入路径的内部预备机制尚保留，普通后台首击/拖动接收问题尚未解决；本修正不宣称完全后台兼容。
