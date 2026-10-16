# macOS 轻量视觉通路与 BetterDisplay 显示器 UUID（2026-09-24）

## 本轮范围与结果

本轮先实现会话 Live 所需的**只读观察底座**，不把一次截图冒充连续 Live 或 Codex 原生叠层。用户已分配的 Paint S 主窗口仍在现有 BetterDisplay「RWI Workspace」虚拟屏；没有新增虚拟屏、改变显示设置、发送 Paint 输入、移动窗口或改写画稿。

1. macOS `computer.capture_visual` 改用 ScreenCaptureKit 的精确 CGWindowID/PID 过滤器，在原生回调内直接请求有界尺寸（64–2048 px），不先做完整 `observe` 的 AX 遍历、Retina PNG 编码后再缩小。开始/结束分别核对用户会话、进程创建时间、窗口身份/几何/标题和显示器拓扑；变化则丢弃画面。响应只含图像与只读元数据，不产生 input frame、租约或输入引用。独立系统面板不在同一张精确窗口图中，须另行发现/绑定。
2. `computer.discover` 的 macOS 页头新增当前 active displays，包含 ColorSync UUID、当次 displayID、是否内建、桌面矩形、像素尺寸和旋转，并标注目录/UUID 是否完整；每个窗口也携带所属显示器。`DisplayTarget` 支持 UUID，可与 id/built_in 同时提供并要求全部匹配，防止旧 id 指向另一块屏。保留 32 显示器上限和 12 KiB 分页预算。
3. MCP 目录说明和 macOS Skill 更新；新 `rwi-mcp` 已部署，插件通过个人 marketplace cachebuster 重新安装为 `0.2.0+codex.20260924114855`。**本任务已加载的 MCP 进程仍展示旧 schema/说明**；新任务加载新版后才可在 Codex 工具参数中直接填写 `display.uuid`。本轮没有对 Paint 发出 UUID 移窗动作。

选择 UUID 的依据是 [BetterDisplay 官方 CLI 文档](https://github.com/waydabber/BetterDisplay/wiki/Integration-features%2C-CLI)明确区分 UUID 和可能变化的 displayID；[开发者在讨论 #3628 的答复](https://github.com/waydabber/BetterDisplay/discussions/3628)指出 CGDirectDisplayID 是临时标识，并指向 Apple 的 [`CGDisplayCreateUUIDFromDisplayID`](https://developer.apple.com/documentation/colorsync/cgdisplaycreateuuidfromdisplayid%28_%3A%29)。UUID 仍需在每次操作前与**当前 active display** 匹配，不能把它当作独立会话/安全域。

## 按时间的实际验收

| 顺序 | 通路与事实 | 结果 |
|---|---|---|
| 1 | 原生 Plugin `computer.targets`：重部署后 Agent PID 45416，accessibility/screen_capture/post_event/listen_event 均 true；Mac `capture_visual.implemented=true` | Agent 与 Host 重连成功，权限未丢 |
| 2 | Plugin `computer.discover(pid=14404)`：内建屏 id1 / UUID `37D8832A-2D66-02CA-B9F7-8F30A301B230` / `[0,0,1512,982]`；RWI Workspace id7 / UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41` / `[1512,0,1920,1080]`；`display_catalog_complete=true`、`display_uuid_complete=true`。Paint 主窗口 `cgwindow:2189` 和参考窗 `cgwindow:3037` 均归 id7 | 与先前 BetterDisplay Console 所报虚拟屏 UUID 一致；当前坐标不能沿用历史 x2946 |
| 3 | 精确绑定主窗口（PID14404、native epoch `6b4e6fa7…`），直接 `capture_visual(max_edge=1024)` | 1024×471 PNG，源窗口 `[1517,7,1434,660]`；`frame_created=false`、`input_authority_granted=false`、`ax_walk=false` |
| 4 | 先前同窗口 5 次只读样本：512边缘 83/45ms，1024边缘 84/64ms，2048上限94ms；普通 `observe(inline_image=false)` 3 次 547/523/480ms | 仅同机/同窗口顺序样本，不是跨版本或多场景基准；`observe` 会创建短时 input frame，功能范围不同，不据此宣称一般性能优势 |
| 5 | 最终重部署后再次 Plugin `discover/bind/capture_visual`：139ms、1024×471、PNG 163010 B，仍指向 UUID `13E4…BD41` | 最终安装版本实机通路通过 |
| 6 | 原生画稿快照 SHA256 重查 | `b805184358b19b47e4a293f003a71349a4c044b4c40bf28ddcd84ce7cef53d23`，与操作前完全一致 |

实际原样截图：[01 Paint 窗口视觉帧](../../outputs/validation/2026-09-24-live-visual-display-uuid/01-paint-window-visual.png)，1024×471、162974 B、SHA256 `2b67fe4a94e8e32a75a3d2b28342a1865d1446a55ff6901e9c3af222179ae242`。这是部署中段从**已加载 Plugin MCP** 的响应图像原样保存并复核 SHA 的单帧；最终部署后同一通路也成功，但该截图不是最终调用的那张图。画面显示 Paint S 可编辑图层界面，没有合成重绘。

## 门禁与部署

- `cargo fmt --all -- --check`、`cargo test --workspace --locked --offline`：**475 passed / 10 ignored，38 suites**；`cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`、`git diff --check` 均通过。新 `gateway_stdio` 测试确认 MCP 目录说明与 `move_window.display.uuid` schema。
- Release `rwi-agent` SHA256 `f1777cb7aa955b0cdf614116f4c030534581bbabacc47c2ef057ec878b437c7f`；原有本地证书 `RWI Local Development` 重新签名、严格验证后替换同路径 Agent，LaunchAgent 重启。新 `rwi-mcp` SHA256 `da61d4a91a623135ba50632db40ff72f7d457a8471e5d8bbdc2791affff56855`；原路径原子替换，未改配对/profile/journal。
- 部署备份与临时文件在忽略的 `work/live-visual-20260924/`；画稿快照未动。未提交或推送。

## 未完成的 Live／跨屏工作

`capture_visual` 是模型按需获取单窗口帧的底座，**不是**用户所见 Codex Computer Use 的会话叠层。仍缺：从会话根窗口可靠关联系统面板/跨 PID/跨显示器的原生关系证据；任务级观察订阅的启动/暂停/关闭；Host 能否承载第三方 Live UI 的契约；隐私范围和并发任务隔离。固定 BetterDisplay PIP 只镜像一个显示器，Colors 面板可能跑到内建屏，见[独立前台/面板实测](</Users/vincentadamnemessis/.codex/worktrees/bf80/Real World Interaction/docs/validation/2026-09-24-betterdisplay-foreground.md>)。下一步应以显式会话根身份构造候选窗口图，关系不明时请求选择，不能默默捕获整块主屏或把其他任务窗口纳入。

Paint V14 本轮保持第三行企鹅头像 50×48 和 130 图层对象；尚需宽度精调与其余复刻工作。
