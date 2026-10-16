# Paint S Live 与截图清晰度验收（2026-09-25）

## 问题和修正

用户反馈 Live/截图中的小字发糊。先前 Live 曾把整个 Paint S 窗口压到 640px；后续即使改为 1024px，在 Retina 面板上仍被放大显示。macOS 的 ScreenCaptureKit 提供逻辑点尺寸和 `pointPixelScale`，旧限幅路径误把逻辑点尺寸当作实际像素来源。本轮改为按实际 native pixel scale 计算 bounded capture；Live 与 `computer.snapshot` 默认上限改为 2048px，均不向上插值。保留 `computer.image` 对原图像素的裁切路径，小字可取无损局部图。PNG 编码本身无有损压缩。

## 按时间顺序的真实 Host 验收

1. 用户重启 Codex 后，当前任务重新发现并绑定 BetterDisplay 虚拟屏上的 Paint S：PID `14404`，主窗口 `cgwindow:2189`，显示器 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`。Agent 四项 TCC（辅助功能、屏幕录制、输入监控、事件发送）均为 true；没有再次请求授权。
2. 早期 Live 为 640px 源/420×284pt 面板，随后 1024px 源/1040pt 面板仍在 Retina 上放大。修正后，原生 Agent Live 状态实读 `source.image_size=[2048,942]`、面板 `bounds=[2368,455,1040,577]`；`frames_presented` 持续增长，`presenter_visible=true`、`presenter_image_present=true`、`last_error=null`。一次读回的 `last_capture_ms` 为 155–162ms；仅为单次采样，不是吞吐基准。Live 位于虚拟屏，不占内建显示器。
3. 用真实任务切换复核新版 Live：切到另一任务后 `presentation_suspended=true`、原生面板不可见且 `image_present=false`；返回本任务后恢复显示，源仍为 2048×942。此为真实 Agent 面板验收，不仅是网页 `visibilityState` 探针。任务内浏览器面板被关闭/遮盖时也隐藏；不声称存在 Codex 私有 task-ID API。
4. RWI `computer.snapshot(max_edge=2048)` 返回可映射的 2048×942 PNG，保存在本地 [2048px 预览](../../outputs/validation/2026-09-25-paint-s-2048.png)（410422 bytes，SHA-256 `8a2febbb335e3ff2153f6150d80fc3701bccb43128b5f81600662fa732d48703`）。`computer.observe(inline_image=true)` 另取未缩放的 [2868×1320 原生截图](../../outputs/validation/2026-09-25-paint-s-native-2868.png)（581338 bytes，SHA-256 `d643565c55cf638e99be5c1291f1498142801d23ba2e94b13c9b62552e619706`）；独立安装版 MCP 在**省略** `max_edge` 后也实际返回 2048×942，映射明确记录 `original_size=[2868,1320]`、`view_size=[2048,942]`。Paint S 工具栏和图层栏文字比旧缩略预览清楚；微信图像内的部分极小字来自原始素材，不应声称被算法恢复。两图仅用于本地验收，不纳入 Git。
5. 同一工作流中，1200px 全窗预览单次约 600ms（observe 520ms、image 70ms）；2048px 单次约 610ms（observe 469ms、image 125ms）。通过 `computer.result` 找到原始 artifact，再取 `[1020,455,540,480]` native crop 约 52ms，局部的对话名可读。以上是不同单次运行环境样本，不构成 A/B 性能优越性结论。全设备 `computer.targets` 约 5s 主要来自离线 Windows 超时，不能归因于 Mac 截图。
6. Paint “Resize canvas and image” AXPress 曾返回 `native_action_unconfirmed`，但**独立新发现**确见真实 Resize 弹窗；没有重放原请求。绑定精确弹窗后截图可读、Cancel 派发并复查弹窗消失。此为普通可视弹窗，不能冒充 semantic-only 逻辑 AXDialog 的实机验收。

## 实现、部署和边界

- `rwi-core` Live 默认/上限现为 2048；`rwi-macos` 使用 native `pointPixelScale` 限幅 ScreenCaptureKit 输出，Live 面板按 Retina 物理像素显示；`rwi-mcp` 新安装进程的 `computer.snapshot` 默认 2048（上文已实际新进程闭环）。**本任务旧 MCP gateway 仍是热加载旧代码**，因此这里显式传 `max_edge=2048`；Agent 新实现已生效。
- 已原子部署 Agent 与 MCP，安装 Agent SHA-256 `35bbccd5ba0e520db3f01857ea82c1cf44fe03803cbb7aaae7411076ec97bf69`，安装 MCP SHA-256 `91ef86d4e274cf5f5950a0cb2b69186bdf69ede5eda75a0110cbabcdd0db9f3b`。本机代码签名 designated requirement/证书 SHA-1 `24B346A95EC8F450F7D8433A2E1876568DBCB2F8` 未变；生产 TLS 配对/profile 未更改。Plugin 缓存破坏版本 `0.2.0+codex.20260924163936` 已安装，供新任务加载。
- `cargo fmt --all -- --check`、`cargo test --workspace --locked --offline`（507 passed / 10 ignored）、`cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`、`git diff --check`、Agent release 构建均通过。未提交/推送。
- 两份已保存的可编辑 Paint 源仍各 446815 bytes，SHA-256 `1573f4f74b45a6c13014dd9c0efeb1d2d3a2185f9fa4368edab74c91ba8a9998`。本轮一次微小拖拽仅获派发回执，画面改变未独立证实，未再次保存；不把它计入 V14 进度，也不盲目 Undo。
- 后续仍需验证 semantic-only 弹窗、Live 与长操作的争用，并继续 V14 图层源复刻。小字若在原始素材中已经低分辨率，需换高分辨率源/重绘；放大预览不会产生新的真实细节。
