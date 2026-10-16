# F133 独立实机验收（2026-10-16，Asia/Shanghai）

**主验收通过：真实 Codex Host 已显示 PNG-data 图片。** 默认模式完成显示 → 隐藏清图 → 恢复新图 → 关闭终态。纯 Host 可选模式与条件性任务切换未验，未混入通过结论。

## 绑定与执行
- 唯一根：`/Volumes/ChatGPT/Real World Interaction`，与注册项目路径 samefile。
- 原样执行现成 `COMMAND_METADATA.json` 的 command 数组；新一次性 F133，真实 exec session `17997` 保留至清理，退出 `0`，未重放 F129。
- 自有 `RWI Live 133`：PID `94543`、窗口 `cgwindow:3642`、epoch `8321f87db6ce327154028451be94e47aefc27a9d32750062c6b6392e4f089f7a`。精确 PID discover 与 attach 在同一批、10 秒目录 TTL 内完成。
- 源屏 `4 / 13E4E9B0-47F2-4043-9CDC-878D5441BD41`；原生预览屏 `1 / 37D8832A-2D66-02CA-B9F7-8F30A301B230`。duration `240000`、interval `500`、max_edge `2048`。
- Agent `47164 / 8c2d9f8f-a499-4aae-8b7c-850d276500ce` 保持不变。

## 分模式结果
| 模式 | inline_pixels_verified | 隐藏 | 恢复 | 关闭 |
|---|---|---|---|---|
| 默认：Host 图片 + 原生浮层，`live:7` | **true**，1320×1104 | 通过 | 通过，新 capture 时间 | 通过，stopped / worker_done |
| `host_image_only` | 未验 | 未验 | 未验 | 未验 |

Agent 的确广告 `host_image_only` 支持，但本任务加载的 `computer.show_live` / `computer.live` typed options schema 未公开这个字段（原样记录于 `MCP_EVIDENCE.json`）。没有擅加未知字段、借私有 API 或另起默认会话冒充此模式；这不等于实际调用被拒绝，也不判定 Agent 不支持。

## 主验收原截图：按实际顺序
以下为 CUA MCP Apps 返回的原 JPEG，未裁切、重编码或伪造。DOM 只记录 src 是否存在及 scheme，未输出完整 data URL/base64。每张文件已重新打开核验。

### 1. 真实 Host 出图
`naturalWidth=1320, naturalHeight=1104, hidden=false, src_present=true, src_scheme=data:image/png;base64`；图像 bounds `286.953125×240`，display=block。原生 status 独立匹配源 PID/window/epoch/屏 UUID/image_size。原生浮层读回 `on_requested_display=true`，仅作为交叉证据，非 Host 图片的替代。

![01 默认真实图片](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f133-fresh-host-png-data-live-acceptance-2026-10-16/01-default-visible.jpg)

### 2. 点击「隐藏预览」
曾实际显示的图像随后 `src_present=false, hidden=true, naturalWidth=0, naturalHeight=0, display=none`；原生 `presentation_suspended=true, presenter_image_present=false, presenter_visible=false`。这是组件隐藏控制验收，不是自动任务切换。

![02 隐藏清图](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f133-fresh-host-png-data-live-acceptance-2026-10-16/02-default-hidden.jpg)

### 3. 点击「恢复预览」
重新看见 1320×1104 的真实 Host 图片，src 恢复为 PNG-data；原生 capture 时间 `1792119632857` 晚于隐藏前最后源图 `1792119614118`，原生浮层也恢复且仍在请求屏。

![03 恢复新图](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f133-fresh-host-png-data-live-acceptance-2026-10-16/03-default-restored.jpg)

### 4. 点击「关闭预览」
稳定 DOM 为「本轮画面已清除」，src 移除、hidden=true。原生 `status=stopped, worker_done=true, presenter_image_present=false, presenter_visible=false, capture_pending=false, native_call_pending=false`。关闭后的观察未恢复、未重放。

![04 关闭终态](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f133-fresh-host-png-data-live-acceptance-2026-10-16/04-default-closed.jpg)

## 时间线（原生工具采样，非原子组合）
| 上海时间 | 事件 |
|---|---|
| 10:59:11.510 | `default_show_live` |
| 10:59:50.409 | `default_pixels_visible_native_status` |
| 11:00:19.035 | `default_hidden` |
| 11:00:33.176 | `default_restored` |
| 11:00:42.668 | `default_closed` |
| 11:01:35.458 | `fixture_close_once_AXPress` |
| 11:01:39.302 | `cleanup_original_operation_query` |
| 11:01:39.316 | `cleanup_detach` |
| 11:01:55.600 | `cleanup_targets_final` |

## 清理及边界
- 所有本轮 Live 已终止；以新快照中唯一 `Close window / AXPress`、稳定请求 ID `F133-live-own-94543-3642-close-20261016-once` 关闭自有 fixture **一次**。查询原 operation：dispatch returned、terminal/resources_released，replay forbidden；receipt 本身不当作退出证明。
- 独立生命周期证据：fixture `closed`、真实进程退出 `0`、PID 不存在、lifetime_close_requests `0`，目标已解绑。普通业务输入计数均为 `0`。
- 原前台精确身份 PID `11924` / `cgwindow:421` / process_start `11924:1792037871:796730` / epoch `4e39439da718e4fd2d0418cc17d01dc45188ff11774f9174e31d58c05f6931bf` 前后相同；fixture 环境历史始终未成为前台。
- 源 484 文件及 protected12 重新核 hash 均未变。未改源码、既有四角色、continuation-controller、Host/TCC/证书/CA/私钥；未部署/提交/推送、未发其他会话消息、未请求用户配合。
- 两模式均实际出图是公开任务切换用例的前置条件，未满足，因此 **未调用 navigate_to_codex_page**，未验证此路径的自动隐藏/恢复，更未声称所有 Host 的强任务 ID 绑定。
- 未读取 Codex 真实 CSP 头；本轮成功不倒推先前 Blob 错误的唯一根因。未运行或冒称新一轮源码回归测试。
- 两次观察工具错误已如实保留：MCP Apps 不接受 fullPage=true；随后保存未赋值变量失败。改用 `screenshot({})` 获得并保存原 JPEG，无业务输入重放。

## 机器可读证据
- /Volumes/ChatGPT/Real World Interaction/work/f133-fresh-host-png-data-live-acceptance-2026-10-16/RESULT.json
- /Volumes/ChatGPT/Real World Interaction/work/f133-fresh-host-png-data-live-acceptance-2026-10-16/MCP_EVIDENCE.json
- /Volumes/ChatGPT/Real World Interaction/work/f133-fresh-host-png-data-live-acceptance-2026-10-16/LIFECYCLE_RESULT.json
- /Volumes/ChatGPT/Real World Interaction/work/f133-fresh-host-png-data-live-acceptance-2026-10-16/PID_EXIT_READBACK.json
- /Volumes/ChatGPT/Real World Interaction/outputs/validation/f133-fresh-host-png-data-live-acceptance-2026-10-16/CUA_EVENTS.json
