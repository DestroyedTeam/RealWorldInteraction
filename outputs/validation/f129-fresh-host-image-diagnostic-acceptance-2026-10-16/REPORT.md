# F129｜新会话 Live 图片错误诊断验收

> 结论：诊断版已在真实 Host DOM 加载，精确返回 **`decode_png / rpc_0`**。**内联像素未验证成功（`inline_pixels_verified=false`）**。该短码未揭示更细解码原因；不能据此推断 CSP、Blob 或 PNG 内容根因。

## 已观察结果

- 组件：`RWI Live · 会话内预览`；实际资源 `ui://real-world-interaction/live/v4.html`。
- 首次 UTC：`2026-10-16T02:46:23.863Z`；原文：**画面暂不可用 · 已清除旧画面（decode_png/rpc_0）**。
- 初始及恢复后均为 `naturalWidth=0 / naturalHeight=0 / hidden=true / src不存在 / bounds=0×0`；有 img 节点、资源 URI 或组件连接均未计为出图成功。
- 自有源固定为 PID **85304**、`cgwindow:3556`、native_epoch `bc542d1861edf978ec0e6167d5e1663cc091fb4ba4c81c716fe21dcdf640cac8`；虚拟屏 **4** / UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`，原生源图 **1320×1104**。
- 原生浮层在内建屏1，实际边界 `[1578,921,198,200]`，`on_requested_display=true`、`presenter_image_present=true`；这不等于会话内 img 出图。

## 有界状态变化（仅一个 Live）

| 动作 | UTC / 实际结果 |
|---|---|
| 读取错误 | 2026-10-16T02:46:23.863Z；`decode_png/rpc_0` |
| 隐藏一次 | 2026-10-16T02:47:46.393Z；DOM 诊断属性清空、img仍无src；原生浮层隐藏且清图 |
| 恢复一次 | 2026-10-16T02:47:57.103Z；再次 `decode_png/rpc_0`；原生源时间戳更新，采集帧175→189 |
| 关闭一次 | 2026-10-16T02:48:08.192Z；随后原生 `stopped / worker_done=true / capture_pending=false` |
| 最终组件 | 2026-10-16T02:49:04.966Z；`关闭请求已返回 · 重新打开需新建观察`；`本轮画面已清除`；双按钮禁用、诊断清空、img无src |

**边界：从未观察到内联图片成功显示，因此没有声称“隐藏删除了一张曾显示的内联图”，也没有声称恢复生成了新的内联图片。**

## 时间顺序真实截图

浏览器截图保留原始 JPEG；原生源同时保留 snapshot 规范化视图 PNG 与公开 computer.artifact 缓存读取的原始 PNG。后者与原始 png_sha256 一致。两者是同次采集，不是额外原生捕获。以下按实际采集顺序排列；JPEG时间标记为保存UTC，未冒充原子捕获时间。复制到交付目录前后 SHA-256 一致。

### 1. 原生自有源：1320×1104；业务计数为0，源窗口正常。
实际采集UTC：`2026-10-16T02:46:45.445Z`；image/png；1320×1104；101397 bytes。
![原生自有源：1320×1104；业务计数为0，源窗口正常。](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f129-fresh-host-image-diagnostic-acceptance-2026-10-16/screenshots/02-native-source-original.png)

### 2. 真实内联错误：页面明确显示 decode_png/rpc_0，但无图片。
保存UTC：`2026-10-16T02:47:12.429Z`；image/jpeg；816×1079；18177 bytes。
![真实内联错误：页面明确显示 decode_png/rpc_0，但无图片。](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f129-fresh-host-image-diagnostic-acceptance-2026-10-16/screenshots/01-inline-decode-error.jpg)

### 3. 隐藏：诊断清空，内联图仍无src；此前未出图。
保存UTC：`2026-10-16T02:47:46.504Z`；image/jpeg；816×1079；16107 bytes。
![隐藏：诊断清空，内联图仍无src；此前未出图。](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f129-fresh-host-image-diagnostic-acceptance-2026-10-16/screenshots/03-inline-hidden.jpg)

### 4. 恢复：再次出现相同精确阶段/短码，没有内联像素。
保存UTC：`2026-10-16T02:47:57.191Z`；image/jpeg；816×1079；18560 bytes。
![恢复：再次出现相同精确阶段/短码，没有内联像素。](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f129-fresh-host-image-diagnostic-acceptance-2026-10-16/screenshots/04-inline-resumed-error.jpg)

### 5. 关闭：按钮禁用，当前画面关闭。
保存UTC：`2026-10-16T02:48:08.274Z`；image/jpeg；816×1079；19625 bytes。
![关闭：按钮禁用，当前画面关闭。](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f129-fresh-host-image-diagnostic-acceptance-2026-10-16/screenshots/05-inline-closed.jpg)

### 6. 最终：关闭请求返回，画面清除；原生 worker 已结束。
保存UTC：`2026-10-16T02:49:05.052Z`；image/jpeg；816×1079；20615 bytes。
![最终：关闭请求返回，画面清除；原生 worker 已结束。](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f129-fresh-host-image-diagnostic-acceptance-2026-10-16/screenshots/06-inline-final-stopped.jpg)

## 清理与未变项

- Live：`stopped`、`worker_done=true`，浮层隐藏且无图，内联src不存在。
- 新快照的 `Close window` 仅执行一次 `AXPress`，请求ID `f129-own-fixture-close-20261016-once`。查询原 operation 得到 `terminal=true / resources_released=true` 后解绑；耐久回执保留。
- 自有 fixture 真实退出 **0**；`ps -p 85304` 返回 **1**、stdout/stderr均空，PID已不存在；保留 exec session 85467 至监督进程退出 **0**。
- 原前台完整身份元组前后相同：PID 11924 / `cgwindow:421` / `11924:1792037871:796730` / `4e39439da718e4fd2d0418cc17d01dc45188ff11774f9174e31d58c05f6931bf`。
- 生产 Agent PID47164、实例8c2d9f8f-a499-4aae-8b7c-850d276500ce及现有权限未变。监督器记录 Source484/protected12 未变。
- 不改源码、四角色、continuation-controller或产品文档；不提交、不部署、不修改 Host/TCC/CA/私钥/证书；不发送会话消息；不启动用户应用或读取用户资料。

## 证据与执行修正

- 机器结果：[/Volumes/ChatGPT/Real World Interaction/work/f129-fresh-host-image-diagnostic-acceptance-2026-10-16/RESULT.json](/Volumes/ChatGPT/Real World Interaction/work/f129-fresh-host-image-diagnostic-acceptance-2026-10-16/RESULT.json)
- 生命周期：[/Volumes/ChatGPT/Real World Interaction/work/f129-fresh-host-image-diagnostic-acceptance-2026-10-16/LIFECYCLE_RESULT.json](/Volumes/ChatGPT/Real World Interaction/work/f129-fresh-host-image-diagnostic-acceptance-2026-10-16/LIFECYCLE_RESULT.json)
- 工具状态、原operation和前台身份：[/Volumes/ChatGPT/Real World Interaction/work/f129-fresh-host-image-diagnostic-acceptance-2026-10-16/LOADED-RWI-02.json](/Volumes/ChatGPT/Real World Interaction/work/f129-fresh-host-image-diagnostic-acceptance-2026-10-16/LOADED-RWI-02.json)
- 完整截图清单/字节哈希：[/Volumes/ChatGPT/Real World Interaction/outputs/validation/f129-fresh-host-image-diagnostic-acceptance-2026-10-16/SCREENSHOT-MANIFEST.json](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f129-fresh-host-image-diagnostic-acceptance-2026-10-16/SCREENSHOT-MANIFEST.json)
- 截图API拒绝 fullPage，已记录原错并改为可见文档截图；单次大字节保存超出64000-byte自动审阅限额，改为三段原始base64缓冲后落盘；均无输入重放、无图片转换。
- 本轮没有修改产品代码，也没有把既往Rust回归当成本轮新测试结果。

## PNG 原始字节补充核验

- 原始 PNG：`/Volumes/ChatGPT/Real World Interaction/work/f129-fresh-host-image-diagnostic-acceptance-2026-10-16/screenshots/02-native-source-original.png`；101397 bytes；SHA-256 `100de8cf730c64a98c1475e3f5967cd51c45ddc11b62f1086cf4a69648126b50`，与公开缓存返回及保留的原始 png_sha256 全部一致。
- snapshot 另行输出的规范化视图保留在 `/Volumes/ChatGPT/Real World Interaction/work/f129-fresh-host-image-diagnostic-acceptance-2026-10-16/screenshots/02-native-source.png`；其不同编码字节未冒充原始PNG，也未覆盖。截图清单含7个字节文件、6个时间序列截图（原生源同次采集有2种表示）。
