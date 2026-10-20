# Codex 重启后的 RWI Host 实际验收（2026-09-25）

## 顺序与结果

1. 当前任务加载的 Plugin 来自 `0.2.0+codex.20260925131605`，Host 当前 `computer.live` schema **直接包含 `hide`**，不再是重启前的旧枚举。`computer.targets(device_id=mac-host)` 返回 macOS Agent `instance=4391ae85-7406-435d-8053-be6e702abc79`、PID40775、服务 healthy、四项权限（辅助功能、屏幕录制、监听事件、发送事件）全为 true，输入域未围栏。
2. `computer.discover(pid=14404)` 重新发现 Paint S 正式主窗 `cgwindow:2189`，标题 `paint-v14-card-cleanup-20260919`，位于 BetterDisplay 虚拟屏 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`、`[1536,24,1434,660]`；按本次返回的 native epoch 重新绑定。没有使用历史 frame/坐标，也没有输入。
3. `computer.live start` 选择同一虚拟屏、`max_edge=2048`、120 秒；初始因 Host 视图未呈现而安全暂停采集。把返回的**本次精确 viewer URL**交给 `open_in_codex` 两次，Host 均返回 `status=queued`；Computer Use 的 in-app browser inventory 报 fetch 失败，不能用它冒充打开了本任务的 viewer。RWI `status` 一直为 `presentation_suspended=true`、`frames_captured=0`、`presenter_visible=false`、`presenter_image_present=false`，原生 readback 为 `on_screen=false/settled=true`。因此**本次没有重新证明“当前任务 Live 打开后可见”或任务切换隐藏/恢复**；这不是截图失败，也不是浮层已显示。
4. 直接调用新 Host MCP `computer.live hide` 成功，返回 `status=hidden`、`user_hidden=true`、无面板/图像且原生 readback `on_screen=false/settled=true`；`resume` 返回 `running`、`user_hidden=false` 并因 Host viewer 未显示继续安全悬停。此步骤只证实新参数与隐藏状态机/安全门控，**不声称在可见面板上验证了视觉恢复**。
5. `computer.live stop` 返回 `stopped`、`stop_reason=requested`、`worker_done=true`、`native_call_pending=false`、无面板/图像、readback `on_screen=false/settled=true`；没有遗留 Live 会话。随后同一正式 Paint 绑定下的 `computer.snapshot(max_edge=2048)` 实际返回源图 `2868×1320`、预览图 `2048×942`、`visual_available=true`，一次调用耗时771ms（observe643ms/image127ms）。再次 snapshot 后，从该次只读原始 artifact 用 `computer.image(crop=[1000,390,420,680])` 取得 `native_resolution_tile`，`view_size=[420,680]`、`view_to_original=[1,0,0,1,1000,390]`，确为1:1像素局部，并在本次工具输出中实际检查。局部文字仍有画稿自身的叠字/残线，不能把绘制问题解释成截图降采样。这是截图清晰度/映射路径的重启后真实 Host 实测，不是 Live 可见性证明。
6. 正式 Paint 文件与阶段快照均为 SHA‑256 `c8b0e73295e8e928c98174e245efe11f59be311c864fd05ec76f6cf2a978f55c`，本次没有编辑画稿、触碰物理主屏或发送键鼠。`git diff --check` 通过；未新改 Rust、未重建/部署、未提交/推送。

## 后续验收条件

需要本任务的 Codex browser panel **实际可见**，并由真实 Host presence 更新解除 `presentation_suspended`；再验 `running/visible/image`、直接 `hide` 的原生窗口消失、`resume` 的图像恢复、`stop` 的释放，以及切换任务的自动清空。`open_in_codex` 的 `queued` 回执不能替代该验收；不得伪造 presence 事件或用外部浏览器替代任务绑定。
