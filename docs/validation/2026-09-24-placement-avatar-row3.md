# BetterDisplay 窗口归位与 Paint V14 第三行头像（2026-09-24）

## 按时间的真实结果

1. **独立 TextEdit 归位，不碰 V14。**专用 `placement.rtf` 窗口 `cgwindow:3681`/PID 24628 起于内建屏 `[152,72,586,488]`。经当前 Plugin 的 `discover → bind → observe → resolve(window) → claim → move_window → release`，唯一请求 `placement-settle-textedit-20260924-01` 将其放到既有 BetterDisplay display 7。回执 `dispatch=returned`、`placement_confirmed=true`、`settle_checks=2`、`settle_elapsed_ms=62`、最终 `[1536,24,586,488]`；独立新 `discover` 同值。随后结束该 TextEdit 进程。此为单次实机确认，不是稳定性或耗时基准。
2. **修正归位回读。**原先 `AXPosition` 后固定 150 ms 即返回一帧，移动动画期间可能得到临时 `display:null`。现在在原有一次设置后，最多 950 ms、每 50 ms 只读检查同一 PID/CGWindowID/进程与显示拓扑；目标显示器上的连续两帧几何一致才报告 `placement_confirmed=true`，超时只报未确认并要求新发现，绝不重发移动。进入 `AXUIElementSetAttributeValue` 前计数，非零 AX 返回不再被误当成已证明零派发。
3. **参考图试错与恢复。**首次试选第三行时 Paint 实际仍在 Ellipse/选择状态，RWI 拖动生成了意外椭圆；CUA `Undo` 后原第三行企鹅头像恢复，原参考 PNG 文件未保存改写（621749 bytes，SHA256 `06b69665aa59fe4d2013a4dcd10577fcec160f147b4c604a45f375a8a567334b`）。RWI 后来明确 AXPress `Rectangular Selection`，独立回读 value=1，再拖出源图 `Left=162, Top=483, Width=82, Height=82`；这次只是选择，没有写入原参考 PNG。
4. **遮挡不是“未知可见性”。**当工作文档盖在参考窗上时，RWI `cmd+c` 在派发前返回 `visibility_unknown/native_dispatch_entered=false`；实际 AXScrollArea 是 `visible=false, occluded=true`。曾试验允许「原生焦点二次证明下的未知可见性」候选，但它不适用于此明确遮挡，已撤回且最终部署不含该放宽。CUA 显式 Raise 参考窗后，RWI 新观察 `visible=true, occluded=false`，新请求 `v14-0924-row3-copy-reference-02` 返回；未重放原拒绝请求。
5. **独立可编辑 Image。**CUA 通过 Paint Window 菜单切回工作文档；RWI `cmd+v` 后独立截图显示新增第七个 Image，初始 82×82。RWI 用图像控制柄缩至 50×48；一次极短微调虽返回但未见尺寸变化，随后 CUA 发现并关闭 Paint 首次缩放提示。不能把那次回执算作修正成功。CUA 结束 Rotate & Resize 与 Editing，RWI 将该图层从卡片上方拖至第三行头像位，独立截图确认企鹅头像覆盖旧占位；当前宽仍比目标 48 多 2 像素。
6. **实际保存，而非输入回执。**此前源文件 438740 bytes/SHA256 `f2b2d4e5615b24649355ceaba7cc5f514bae0b65f52d94ebf888e1db1f413f1c`。RWI 在工作窗已由 CUA 前置、且结束编辑后派发唯一 `cmd+s` 请求 `v14-0924-row3-save-01`；磁盘实际变为 **446815 bytes**/SHA256 `b805184358b19b47e4a293f003a71349a4c044b4c40bf28ddcd84ce7cef53d23`。只读解开两层 NSKeyedArchiver：`ObjectsCount=130`，7 Image/27 Rectangle/75 Line/11 Ellipse/7 RoundRectangle/3 Text，原生源文件没有扁平化。新增快照 `outputs/paint-v14-source/paint-v14-20260924-avatar-row3.paint` 与工作源文件字节一致；旧快照保留。此成功证明当前前台/编辑态条件下 RWI 快捷键真实落盘，**不能外推为完全后台保存已修复**。

按时间保留的实际截图：[上阶段第二行头像](../../outputs/validation/2026-09-24-virtual-workspace/03-avatar-final.png) → [本阶段第三行最终画面](../../outputs/validation/2026-09-24-placement-avatar-row3/01-row3-final.png)。后者是对已分配 Paint 工作窗口 `cgwindow:2189` 的原样 2868×1320 单窗口截取；RWI 同时独立 snapshot 见到同一位置。没有把示意重绘充作实机图。

## 最终部署与仍待事项

- 最终 `cargo fmt --all -- --check`、workspace locked offline **471 passed / 10 ignored（38 suites）**、all-target/all-feature clippy `-D warnings` 通过；release Agent 用同一 `RWI Local Development` 证书重签部署，最终 PID 32328、安装 app 可验证，二进制 SHA256 `40bd30f0146b3f5a9021814821ac1a9e4235be6147b3df12575ad24e222882e8`。Host Plugin 重连后 mac-host 可用，四项权限 true、input fence=false。MCP/profile/pairing/journal 未改。
- 独立调研的 [TextEdit/Colors 跨屏报告](</Users/vincentadamnemessis/.codex/worktrees/bf80/Real World Interaction/docs/validation/2026-09-24-betterdisplay-foreground.md>)仍成立：虚拟屏可承载前台应用，但颜色面板可落内建屏。macOS `computer.activate_window` 依旧 `implemented:false`；本轮只读归位与 V14 编辑没有冒充前台激活能力。
- V14 尚未 1:1：第三行头像 50×48 需精调；更多头像、侧栏/顶栏样式、字体边框与导出对照待完成。系统面板跨屏、会话级 Live/隐私、动态选框误拒绝仍未修复。Paint Undo 栈在会话中保留，但原生文件跨重启历史不保证。没有提交/推送。
