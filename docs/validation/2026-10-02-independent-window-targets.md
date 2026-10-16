# macOS 独立窗口工作集：第三轮候选（未部署）

日期：2026-10-02。继续[真实会话审计](2026-10-02-macos-computer-use-comparison.md)及[保留AX对象候选](2026-10-02-retained-ax-target-revalidation.md)。本轮全部GUI启动、激活、截屏、输入为0；Paint219对象源不变，不操作游戏或飞书配置。

## 解决的问题

此前两个 `rwi.window(sameTarget)` 只是共享一个当前选窗的包装器，不能同时保留主窗、浏览器另一窗口和系统面板。反复bind覆盖主窗、清空frame，也会改动Live源；模型把包装器误当独立窗口，造成失效引用和多余发现/绑定。

本轮新增两个直接MCP工具及明确的调用标题，不要求临时JS：

- `computer.attach_window {target: originalDesktopTarget, selection: freshExactSelection}`：返回新的固定窗口target。每个target拥有独立backend、frame/ref、retained AX对象registry及Live session；原target绑定与Live源不变。使用新target调用snapshot/controls/click/type等既有工具。不是启动/打开原生应用或授权新桌面，不激活、不放置、不采图、不发送input。
- `computer.detach_window {target: attachedTarget}`：先释放该target租约；再销毁绑定、Live及frame/ref/view。并非关闭原生应用窗口；原始PNG artifact仍受原context有界缓存约束，durable操作回执保留，不能重放。root不能被detach；关闭context统一回收。

JS可选：`await desktop.attach(selection)` 返回新target的wrapper；`await window.detach()` 销毁绑定。同target不同JS变量仍共享coordinator，并不是另一原生窗口。JS包装器的原有64个target上限保留；server每个context最多16 targets（root＋15 attachments）。

## 身份、权限与生命周期

1. Rust Backend只增加默认unsupported的attachment工厂；返回backend必须与原target完整Identity一致。没有在platform-neutral transport导入macOS FFI。Windows尚未实现该工厂，能力表明确不宣称支持。
2. Mac attach复用原bind的精确catalog/window/process校验：10秒发现catalog、确切CG ID/PID、同UID出生fingerprint、当前geometry/title/display/session，不能按标题或同PID猜选。发现epoch仍是观察fingerprint，不是OS window birth证明。非标准窗口及semantic-only面板沿用原有准入规则；没有扩大跨PID delegate。
3. Attach不走旧bind的可选place_on_bind及Live source capture分支。新NativeMacDesktop固定binding，不能被bind替换或派生嵌套attachment；通过root发现后再attach另一个。NativeMac初始化保留进程校验；没有AX全树扫描/新ScreenCaptureKit采集。
4. 所有attachments继承原授权与完整桌面Identity，共享capture busy资源、native事件计数、input resource domain、跨进程lease root。A租约不能给B派发；不同context/principal仍竞争同一物理桌面，绝非多鼠标键盘。Observed包装器的native effect证据继续传入原entered标记，拒绝/未知/围栏与durable no-replay不绕过。
5. Agent维护metadata-only的有界target catalog，prepare及cached读均只接受同authenticated principal/context持有的target；不在async acceptance路径获取执行器native锁。Describe返回所属context最新列表，Gateway更新并记住真实句柄。丢失attach回包时刷新targets/查询原execution，不能自动重试attach或input。
6. Detach持有lease mutex检查本target无有效租约，Agent context gate排除当前native worker；不隐式释放租约、修改durable reservation。删除后新binding使用不重复UUID；旧frames和refs不能指向后来创建的attachment。Context关闭或Agent重启后旧operation ref/context仍无效。

## 按实际执行顺序的离线验证

- 首轮构建发现fixture误调用不存在的Core receipt查询API，以及durable检查插入了另一测试作用域；修正为Core原journal字节/有界日志验证、Agent真实durable查询验证。失败轮次未作为通过。
- Core独立窗口fixture：原root绑定/frame保留、两个窗口同PID/同element ID也不能混frame/ref/lease、root切窗不覆盖附件、固定binding拒绝重绑/嵌套、完整Identity变更拒绝、错误selection/cancel零注册、容量16、detach不放租约、另一个context Engine共享physical lease root、原journal跨detach/Engine重建保留。均不调用OS。
- Agent真实prepare/execute/cached/query：输入授权继承、只读principal可attach/observe/detach但claim被拒绝、跨principal/context/Agent instance拒绝、取消后零attachment、销毁后旧target拒绝、Observed原entered证据传递、durable原请求仍queryable且replay_allowed=false。窗口及input都为明确synthetic fixture。
- 实际MCP子进程＋mTLS synthetic endpoint：attach返回句柄可直接snapshot，targets刷新包含/移除附件，外设备覆盖不产生RPC，单次attach无隐式bind/input。另验证子窗口detach后Host presence页立即410且不发RPC，原root viewer仍可读；这只是Host桥生命周期，不是实际overlay视觉验收。首轮把JS专有preview参数误传typed snapshot，schema正常拒绝；另一次测试误假定Gateway已有readOnlyHint，改为检查真实导出的新调用title与schema；两种测试假设均按实际API修正。这不是Chrome/飞书/原生Live验收。
- JavaScript **50 passed**：3个新增测试覆盖独立coordinator、原root观察不被attach覆盖、同target alias仍共享失效、detach后的alias拒绝、失败/异常返回不bind fallback或重试；已有47个lease/receipt/cleanup/summary回归保留。
- 两个Skill及多窗参考按实际新能力更新，旧Agent/Windows不自动启用；bundled quick_validate均通过，已加载插件cache未改。

最终全仓 **563 passed / 10 ignored（41 suites）**；fmt check、locked/offline all-targets/all-features Clippy `-D warnings`、diff check、release构建均通过。签名候选Agent SHA256 `c0e673cb3a19ceba9385cbd34d6666e98e8713383dfeedb62f796957253073ec`，MCP SHA256 `79e311bf5a8f1531aebf4978dd064ee76be8883de14b7c2986ab67005d15f099`。候选Agent以同一本地leaf严格签名，DR保持原identifier＋leaf，生产PID92469和binary SHA复核未变。Jev guard的credentials/model access被拒，未获得可用advisory verdict，不计为审查通过。门禁和签名产物以本轮[候选manifest](../../outputs/validation/2026-10-02-independent-window-targets/candidate-manifest.json)为准。没有用离线耗时宣称真实Mac单步更快；没有伪造实机截图。

## 下一步与未完成项

- 当前共享Agent/已加载MCP仍是旧版本。Agent、MCP及Plugin必须分别部署/重载后才有新工具和行为；签名候选不等于已安装。已询问共享服务空闲部署及BetterDisplay独立Chrome测试窗口，尚无本轮新授权答复；不擅自重启游戏正在使用的服务。
- 实机需在明确分配的独立Chrome窗验证：主窗＋另一窗＋系统面板分别attach、返回主窗、目标级AX重验证及旧ref负例；Live两源独立/隐藏/关闭，保留实际原图及stage timings。原飞书账号/业务配置和用户游戏窗口不作实验对象。
- 动态capture全树settling、初始AX深层coverage、global HID年龄/前台PID替代真实窗口级活动定位仍是剩余P0/P1。独立targets并不保证后台事件被所有应用接收，也不是多窗自动owner发现或完整Mac适配已经完成。继续按计划推进，不提交/推送、不触发私仓Actions。
