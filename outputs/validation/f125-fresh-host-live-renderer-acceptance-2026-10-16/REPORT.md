# F125 新 Host Live 独立实机验收

**结论：`inline_pixels_verified=false`。本轮验收已执行并清理；内联图片验收未通过，不代表全计划完成。**

- 当前确实加载 `ui://real-world-interaction/live/v4.html`，真实 DOM 标题为“RWI Live · 会话内预览”，不是旧 v3。Agent PID 47164、实例 `8c2d9f8f-a499-4aae-8b7c-850d276500ce` 未变。工具未返回 adapter 包版本，不从磁盘安装文件推断运行版本。
- 仅新建自有 PID 74653 / `cgwindow:3482`，完整标题“RWI 验收 · Receiver · F125-live · PID 74653”；fresh discover→attach 同一批完成，源位于 display 4 / UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`。

## 时间序列（2026-10-16，北京时间）

| 顺序 | 时间 | 操作与实际结果 | 判断 |
|---|---|---|---|
| 1 | 10:32:45 | 新 renderer 可见；img 节点 1，naturalWidth/naturalHeight=0/0、hidden=true、无 src，图片边界 0×0。figure 宽360、高16.5；组件显示“画面暂不可用 · 已清除旧画面”。 | 内联像素失败 |
| 2 | 10:33:04 | 实际按钮“隐藏预览”；DOM 无图，原生 suspended=true / visible=false / image_present=false。 | 隐藏/清图状态通过；非已显示图片的移除回归 |
| 3 | 10:33:25 | 实际按钮“恢复预览”；原生恢复并产生更新捕获；组件仍0/0、hidden=true、无src，原错误复现。 | 原生恢复通过；组件新图失败 |
| 4 | 10:34:20 | 实际按钮“关闭预览”；DOM清图、按钮禁用；原生status=stopped、worker_done=true、capture_pending=false、无可见图。 | 关闭通过 |

原生状态确切 source 为 PID 74653 / cgwindow:3482 / epoch `f4c5cddc49e3e724037eca7220cb9186a8198fc227ecdde76143125f6a61c334`，image_size=1320×1104；原生预览边界 [1578,921,198,200]pt，位于请求的内建屏。各阶段源绑定未换。以上只证明原生状态，不代替组件像素证明；Host PiP、严格任务切换绑定均**未验**。

## 原始错误与边界

组件原文：“画面暂不可用 · 已清除旧画面”。当前 DOM/截图接口没有暴露底层 CSP/Blob/协议异常；原生 last_error、last_error_detail、presenter_failure 均 null，fixture stderr 为空。保留原文，不猜根因、不修源码、不直接调用 app-only 方法、不修改消息/presence。截图接口最初 fullPage:true 返回“只捕获可见文档”提示，改用文档规定的 screenshot({})，原始 JPEG 已保存。另一次证据重开脚本误把 MODIFIED_FILE 目录当文件（exit1），原错保留后已改为目录逐文件只读重开（exit0）。

## 清理与对象稳定性

全部本轮 Live 已 stopped/worker_done/清图。随后对新 snapshot 的自有 Close window 执行一次 AXPress：`f125-own-fixture-close-74653-3482-once`，无重放。独立观察 fixture status=closed、唯一记录 window_closed、实际生命周期/监督 exec 退出0、PID消失，attached target 已解绑。业务输入计数全0；仅上述关闭语义动作。原前台完整身份 PID11924/window421/process_start/epoch 前后相等。Agent身份及四项权限未变。

产品路径 samefile=true；Source484 与 protected12 最后逐项哈希全相等。本轮无源码、角色、TCC、CA、私钥、Host设置、提交、部署或 controller 写入，未向其他会话发送消息。

## 沿用同四角色（只读重开，未写入）

- `MODIFIED_FILE`：/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/MODIFIED_FILE
- `DIFF_FILE`：/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/DIFF_FILE.patch
- `VERIFICATION.txt`：/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/VERIFICATION.txt
- `ROLLBACK.sh`：/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/ROLLBACK.sh

只读检查时已出现 F126 current-status-doc-only 补记，当前角色哈希与历史 F124 frame 不同，不能宣称相同；386 managed 文件与当前 modified-hashes 全相符。此变化不替代本轮 Source484/protected12 一致证据。既有（非本轮重跑）BASELINE/MODIFIED/ROLLBACK/REAPPLIED：renderer=false/true/false/true；Node=20/33/20/33，全部exit0，详见 ROLE_REOPEN.json。

## 时间序列真实截图

### 1. 新版组件已连接，但没有内联图片。

![1. 新版组件已连接，但没有内联图片。](</Volumes/ChatGPT/Real World Interaction/outputs/validation/f125-fresh-host-live-renderer-acceptance-2026-10-16/01-initial.jpg>)

原始格式：image/jpeg；816×1079；SHA-256 `6f89c1e717cf660fa07af5e451d1daa2d5ae3b1cdd5d3abb09d6265050672039`。

### 2. 隐藏按钮后的清图状态。

![2. 隐藏按钮后的清图状态。](</Volumes/ChatGPT/Real World Interaction/outputs/validation/f125-fresh-host-live-renderer-acceptance-2026-10-16/02-hidden.jpg>)

原始格式：image/jpeg；816×1079；SHA-256 `f2b8284e9937ec64d07c17837ae68792362b076c2428f2e19239d0dab824fe0f`。

### 3. 恢复后仍无内联新图（失败证据）。

![3. 恢复后仍无内联新图（失败证据）。](</Volumes/ChatGPT/Real World Interaction/outputs/validation/f125-fresh-host-live-renderer-acceptance-2026-10-16/03-resumed.jpg>)

原始格式：image/jpeg；816×1079；SHA-256 `49950b5551bf154c1b6d9412e83c6d250c82519a89a7e03f1d8c8b6509dbdaea`。

### 4. 关闭后的清图与禁用按钮。

![4. 关闭后的清图与禁用按钮。](</Volumes/ChatGPT/Real World Interaction/outputs/validation/f125-fresh-host-live-renderer-acceptance-2026-10-16/04-closed.jpg>)

原始格式：image/jpeg；816×1079；SHA-256 `2f6c65f7b0fee249bb173f97e9878c96d7f420547283037499098ef6627c7d39`。

### 5. 仅用于获取自有 Close window 的 RWI 原生 snapshot；不是组件像素证明。

![5. 仅用于获取自有 Close window 的 RWI 原生 snapshot；不是组件像素证明。](</Volumes/ChatGPT/Real World Interaction/outputs/validation/f125-fresh-host-live-renderer-acceptance-2026-10-16/05-own-fixture-before-cleanup.png>)

原始格式：image/png；1320×1104；SHA-256 `4d5ab96d30ea9ec48d9e312ca1c5ec1dddf605620072529e62581495badbdd2b`。

机器结果：/Volumes/ChatGPT/Real World Interaction/work/f125-fresh-host-live-renderer-acceptance-2026-10-16/RESULT.json

原始 typed MCP 回执：/Volumes/ChatGPT/Real World Interaction/work/f125-fresh-host-live-renderer-acceptance-2026-10-16/loaded

DOM时间序列：/Volumes/ChatGPT/Real World Interaction/outputs/validation/f125-fresh-host-live-renderer-acceptance-2026-10-16/DOM_TIMELINE.json
