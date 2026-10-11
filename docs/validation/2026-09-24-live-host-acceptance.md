# Live 重载后实机验收与 Paint V14 保存（2026-09-24）

## 结论与范围

本任务重载后实际具备 `computer.live` / `computer.place_window`，已通过加载中的 RWI Plugin 验证主路径；不是 standalone RPC 探针。Paint 主文档仍在 BetterDisplay `RWI Workspace`（UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`，当次 display7），预览明确放在内建屏。未新增显示器、更改显示设置或请求 TCC。

**未宣称完整完成**：V14 仍有其他头像和侧栏细节；RWI Live 是当前 MCP device context 的 Agent 浮层，不是 Codex 内部任务 Live UI。相关系统面板仍需显式绑定；本轮还发现 NSAlert 的 AX/CG 双编号问题，语义操作尚未修复。

## 按时间的现场结果

| 顺序 | 操作与独立验证 | 结果 / 问题 |
|---|---|---|
| 1 | Plugin `place_window`，唯一请求 `paint-v14-20260924-place-tool-01`；当前 Paint 窗口已在目标虚拟屏 | 773 ms：observe533 / resolve0 / claim1 / move234 / release1；2次稳定读回56 ms。仅单调用同屏放置样本，非跨屏性能基准 |
| 2 | Plugin 启动 Live，枚举 Agent 自身窗口并精确截取 | 真实无标准标题栏浮层，主屏 `[1068,650,420,284]`，来源为虚拟屏 Paint。首次截图发现按钮对比度差；CUA/AX 无法读取 Agent 控件 |
| 3 | 修正自定义 AppKit event loop 初始化、Dark Aqua、暂停占位和来源标签，签名部署 | CUA 从5秒超时变为约0.1秒返回 Live AX 树；RWI 也能 snapshot 其控件。此为有限现场样本，不作通用性能结论 |
| 4 | API pause，之后精确截取预览；显式绑定参考图，再看 Live | 暂停清空图像；重新绑定主文档/参考图能切换。RWI Agent 自身窗口不会成为预览来源 |
| 5 | RWI AXPress 预览 Pause；另用 CUA 执行新的 Continue / Close 操作 | RWI 原回执 `native_action_unconfirmed` / AXError -25200，但独立 status 和截图确认已暂停；没有重放。CUA Continue 后帧继续增长，Close 后窗口消失、worker_done=true。保留自进程 AXPress 不确定性，不伪造成功回执 |
| 6 | Paint 企鹅头像右中缩放柄拖动2个原生图像像素；新鲜观察侧栏 | RWI drag 1017 ms，实际 Width48 / Height48。随后 Escape 被 `keyboard_target_not_focused` 在派发前拒绝；发现尺寸提示弹窗在内建屏，不是权限丢失 |
| 7 | 显式绑定截图窗口3850，查看 Live；独立读取 Paint AX/CG 身份 | Live 正确显示内建屏提示；snapshot 失败，因为 AXDialog 是3849而非3850，详见下文 |
| 8 | CUA 新鲜 AX 点击弹窗 OK（不勾选永久隐藏）；RWI Live status | 来源消失后 paused / window_missing / image_present=false；显式重新绑定主文档后恢复并显示虚拟屏来源，不残留旧弹窗画面 |
| 9 | RWI 新请求 Escape、cmd+s，终端只读验证磁盘、另存原生快照 | 实际哈希改变，130个绘图对象保留，第三行图片归档边界为 `[476,361,48,48]`。不是只以键盘回执认定已保存 |
| 10 | 最终字体/来源标签版本的实际预览、pause与stop | 420×284原样截图：暂停为居中占位，图像清空；stop随后窗口消失。另发现 AppKit hidden 回执先于 CG on-screen 消失，已追加有界窗口服务器读回，最终部署验收见下方 |

## 本轮实现修正

- 自定义主事件循环补 `NSApplication.finishLaunching`；初始化期间先设 Prohibited，再切 Accessory，均检查返回值。修复 Agent AX 服务未完成初始化。没有独立测量启动前后系统前台 PID，因此不将其称为“已实证零焦点干扰”。依据：[Apple finishLaunching](https://developer.apple.com/documentation/appkit/nsapplication/finishlaunching%28%29)、[ActivationPolicy](https://developer.apple.com/documentation/appkit/nsapplication/activationpolicy-swift.enum)。
- 明确使用 Dark Aqua，修复按钮黑字/深背景；暂停/错误有可见占位，标签显示来源屏幕及标题。`NSTextAlignment` 根据本机 SDK 的架构枚举定义分别处理 Intel / Apple Silicon。标签按 generation/mode/image/caption 缓存，不在60 Hz tick中反复重置。
- 控制请求最多等待250 ms读取 AppKit presenter 状态；窗口服务器确认另有120 ms上限，start/resume/pause/stop需要两次一致原生位置/存在性读回，status不等待。`presenter_readback` 明确区分 on_screen、bounds、settled和错误；缺失/不完整目录、重复ID、错误PID不制造关闭成功。该读回不截图、不重发UI动作，不改变原生采集回调的资源所有权。
- 首次部署该读回后，stop确实返回 `settled=false`，CG边界从420×284缩到412×278，稍后才消失：新读回正确暴露了默认AppKit收起动画，而非继续把hidden标志当成物理窗口消失。随后对RWI预览窗单独设置 [`NSWindow.AnimationBehavior.none`](https://developer.apple.com/documentation/appkit/nswindow/animationbehavior-swift.enum/none)，不改用户全局动画偏好。前一版未通过的[原始读回记录](../../outputs/validation/2026-09-24-live-acceptance/readback-before-animation-fix.json)保留，不覆盖为成功数据。

## NSAlert 的未解决边界

现场原生证据（同一 Paint PID14404）：

- 可见 CG3850：`[550,130,412,409]`，on_screen=true；Live / capture_visual 可读。
- AXFocusedWindow / AXWindows / AXWindow 命中链均指向3849：AXDialog、AXModal=true，`[625,205,262,259]`；OK按钮属于3849。
- CG全窗口目录也有3849，几何与AX一致，但 on-screen-only目录没有它；`kCGWindowIsOnscreen` 字段缺失，不能误报为读取到false。
- AX应用级中心命中返回3849。它仍不足以把3850与3849等同为同一个原生窗口，未通过同PID/相近几何放宽绑定。

忽略的只读 Rust 诊断现要求显式 `RCU_DIAG_PID`，修正布尔字段读取并增加两套几何/命中链输出。[原始限定PID诊断](../../outputs/validation/2026-09-24-live-acceptance/paint-dialog-native.txt)。它不是 Host 输入验收。Apple 的[CG窗口枚举](https://developer.apple.com/documentation/coregraphics/cgwindowlistcopywindowinfo%28_%3A_%3A%29)与[accessibilityWindow](https://developer.apple.com/documentation/appkit/nsaccessibility-c.protocol/accessibilitywindow)定义不同观察通路；目前没有查到足以证明该双编号原生归属关系的公开文档。后续需明确分离语义目标与可见采集目标，不能把缺失映射误当成新TCC需求。

## 可编辑源文件

快照：[paint-v14-20260924-avatar-row3-48x48.paint](../../outputs/paint-v14-source/paint-v14-20260924-avatar-row3-48x48.paint)，446815 bytes，SHA256 `1573f4f74b45a6c13014dd9c0efeb1d2d3a2185f9fa4368edab74c91ba8a9998`。工作源文件字节一致；旧快照未覆盖。

只读解开两层NSKeyedArchiver确认：7 Image、27 Rectangle、75 Line、11 Ellipse、7 RoundRectangle、3 Text，总计130。48×48图片仍是独立Image，不是扁平截图。未证明 Paint 的 Undo 栈跨重启持久化；保留文档及版本快照，不冒称完整操作历史已存入格式。

## 实际截图

1. [初版按钮对比度问题](../../outputs/validation/2026-09-24-live-acceptance/01-before-contrast.png)
2. [API暂停后的真实画面](../../outputs/validation/2026-09-24-live-acceptance/02-api-paused.png)
3. [切换到参考原图](../../outputs/validation/2026-09-24-live-acceptance/03-follow-reference.png)
4. [跨屏提示弹窗预览](../../outputs/validation/2026-09-24-live-acceptance/04-live-cross-screen-alert.png)
5. [Paint头像尺寸48×48](../../outputs/validation/2026-09-24-live-acceptance/07-paint-avatar-48.png)
6. [最终来源标签和可读按钮](../../outputs/validation/2026-09-24-live-acceptance/08-live-final-running.png)
7. [最终暂停清空及居中占位](../../outputs/validation/2026-09-24-live-acceptance/09-live-final-paused.png)
8. [最终部署后的Live](../../outputs/validation/2026-09-24-live-acceptance/10-deployed-live-running.png)
9. [最终部署后的暂停](../../outputs/validation/2026-09-24-live-acceptance/11-deployed-live-paused.png)
10. [保存后的Paint文档及48×48侧栏](../../outputs/validation/2026-09-24-live-acceptance/12-paint-saved-48x48.png)

均从真实 Plugin 返回的图像原样保存，未重绘。第6/7图采自 Agent80887 / cgwindow3859，分别 SHA256 `1a8f83bf0069c0fa7f6fa3dbc781db1f47079558c5d57b79ad2f5772c9ec7918`、`a5804169ba85e69266d9da38113169c1b7085c57c121b12aa1288cab804b30f4`；在窗口服务器读回补丁部署前拍摄。第8/9图来自最终Agent86935 / cgwindow3863，其来源与SHA记录于下方JSON。

## 验证 / 部署

- 最终源码：fmt、workspace locked offline **483 passed / 10 ignored（38 suites）**、all-targets/all-features Clippy `-D warnings`、diff check、release构建全部通过。新增两项纯离线回归覆盖窗口服务器身份/目录完整性与AppKit hidden先于CG消失，默认测试无实机枚举或GUI输入。
- 最终部署Agent PID86935，实例 `ce37a10f-79d1-4f65-ae13-0bf97f480118`；二进制SHA256 `45fdf4c21756476d6db4de5b2f468e35256c458966badd86fdfa1a5d00be97f6`。同一 `RWI Local Development` 签名、严格验证通过；加载中的Plugin读回四项TCC及截图API均true。保留旧bundle、配对、profile、journal；未修改其他应用。
- **最终实测通过**：start / pause / resume / stop 均 `presenter_readback.settled=true`；窗口服务器读回分别16 / 21 / 16 / 31 ms（不是完整工具耗时）。start首帧采集86 ms，pause实际截图清空，resume从28帧增至29帧；stop返回on_screen=false、image=false、worker_done=true、native_call_pending=false，紧接着独立discover的Agent窗口清单为空。消除了本轮观察到的收起动画残留；不外推为所有macOS版本的通用保证。
- [最终安装版本的原始回执和截图元数据](../../outputs/validation/2026-09-24-live-acceptance/final-deployment-acceptance.json)；[此前逐项验收回执](../../outputs/validation/2026-09-24-live-acceptance/acceptance-metadata.json)。最终重新绑定Paint主窗口，Live已停止，文档保持打开。
- Jev部署咨询服务限流，未获得该服务的许可/完成审查结论；部署依据现有明确用户授权、范围核查和本地门禁，不将限流包装为审查通过。
- 未提交或推送；未升级MCP工具表，因此无需为本轮Agent修正再次重启Codex。下一优先级：AX/CG分离面板的明确语义绑定、自进程AXPress回执不确定性、V14其余头像/侧栏，以及后台保存的更多场景实测。
