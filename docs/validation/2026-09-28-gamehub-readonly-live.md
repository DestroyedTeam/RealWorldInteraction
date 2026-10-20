# GameHub 启动的 Wine 游戏：主屏只读观察（2026-09-28）

**后续状态**：用户切回任务后确实呈现了 Live，继而发现预览误落主屏和采样节奏过快；两项均已修复、同证书部署并真实复验。见[跨屏与节奏时序](2026-09-28-gamehub-live-cross-display-fix.md)。下方“0帧/未显示”专指第一次任务面板尚未出现的历史会话。

## 用户授权与边界

用户明确确认：GameHub 和它启动的游戏**保留在内建主屏供用户使用**；RWI 可观察、使用 Live，并可尝试不会明显打断使用的简单操作。本轮为避免影响正在运行的游戏，**只读发现、绑定、截图、Live 状态及像素引用解析**；没有移动窗口、改变前台、点击、发键盘或操作游戏菜单。Live 的预览显示器单独选择 BetterDisplay 虚拟屏 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`，不是游戏源屏幕。

## 按时序实机结果

1. `computer.targets(device_id=mac-host)`：macOS Agent `8d23148e-c118-4ab3-a0ce-27951f8ca6b1` 健康，AX/ScreenCapture/PostEvent/ListenEvent 四权限为 true。精确 PID 查询：GameHub 46002、Wine Steam 50278 无 on-screen CGWindow；实际游戏 PID 51946 有 `CreatureCurios` CGWindow 11351，`[0,0,1512,982]`，属于内建屏 UUID `37D8832A-2D66-02CA-B9F7-8F30A301B230`。首次连续查询其他 PID 后，旧 catalog 中游戏窗已不在，`bind_window` 返回 `window_not_discovered`；**重新发现实际 PID 并立即绑定**后成功，`activated=false/input_sent=false`。调用方不可将启动器 PID 与游戏窗口当成同一原生身份。
2. 对**已绑定的游戏窗口**执行 `computer.capture_visual(max_edge=2048)`：ScreenCaptureKit 精确窗口成功，原像素 **3024×1964**、输出 **2048×1330**，源逻辑矩形 `[0,0,1512,982]`，`ax_walk=false`、`frame_created=false`、`input_authority=false`。这不是全屏或其他应用截图；独立 `lsappinfo` 仍确认 Wine 游戏 PID 51946 保持前台。一次调用的工具墙钟约0.6秒，不能推断普遍延迟或帧率。
3. `computer.snapshot(max_edge=2048)` 同样返回游戏原像素与预览，**`visual_only=false`、`input_reference_available=true`，但 AX 控件数为 0**，`native_complete=unknown`。说明此 Wine 游戏暴露了可匹配的 AX 窗口根，而没有可供语义点击的子控件；不能把它误记成先前实现的“AX 完全不存在 → visual-only”实机分支。`computer.image` 返回[原像素 650×360 局部](../../outputs/validation/2026-09-28-gamehub-readonly/01-native-ax-ui-tile-650x360.png)，字形清晰；一个只读 `computer.resolve_view` 得到该窗口的像素引用与桌面点 `[50,50]`，**没有据此派发输入**。像素引用也不证明游戏会接受模拟事件。
4. `computer.live(start)` 指定**虚拟屏预览**、1秒间隔、2048px、90秒上限，返回 `live:1/status=running`；但 `open_in_codex` 对当前任务返回 `queued`，说明 task-owned 浏览器面板此刻未可见。真实设备状态始终 `presenter_visible=false`、`frames_captured=0`、`frames_presented=0`、`presentation_suspended=true`；90秒后 `duration_expired` 自动停止。**没有把“Live API 启动”写成“用户看到了实时画面”**。已请求用户在方便时短暂切回本任务，以便后续重新启动并验收实际可见的 Live；不通过伪造浏览器可见性绕过任务绑定。

## 产品结论与接续

- 当前 RWI **已在真实 GameHub 子进程游戏上完成精确窗口截图和 AX 根观察**，且没有抢焦点或移动主屏游戏。Launcher/Steam 自身无可见窗口，不代表子进程不能观察。
- AX-less `visual_only=true` 分支仍需另一个确实无 AX 窗的靶场；本游戏不能代替该负例。零控件但可像素映射的窗口，应在模型指引中和“完整语义控件可用”区分。
- Live 源窗口绑定/虚拟屏目标已配置过一次，但当次 task-owned viewer 未显示；**实机 Live 画面、跨任务隐藏/恢复和对游戏帧率影响尚未验收**。下一次用户回到任务后重新启动 90 秒会话，读取 `frames_captured/frames_presented/presenter_visible` 与虚拟屏原生窗口，再按用户使用情况决定是否尝试低扰动输入。
