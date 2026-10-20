# 只读视觉通道与输入观察并列

`computer.capture_visual` 是独立、非 grounding 的截图能力；不是新的任务流程。
Windows 设备端使用 Rust/GDI。它不运行 UIA 树遍历，也不重画反馈图层。

| 能力 | 用途 | 是否产生可输入的 frame |
|---|---|---|
| `computer.observe` / SDK `snapshot` | 元素、焦点、像素与后续输入的原生依据 | 是，仍有严格新鲜度/授权/重验 |
| `computer.capture_visual` / SDK `captureVisual` | 进度查看、低开销截图、真实 Pointer 反馈观察 | 否；`visual_id` 不能 resolve/act |
| 既有 SSH/terminal | 部署、文件/进程检查和执行，与 GUI 交叉验证 | 不产生 GUI 输入权限 |

## 设备侧处理

1. 核对已选择的窗口和当前 host/user/session/seat/desktop，检查原生几何、DPI、
   可见性及显示器。读取权不依赖输入 grant 的短期 TTL。
2. 复制该矩形内**实际可见桌面**的 RGB；后台窗口上覆盖的其他内容仍会被看到，
   不冒充隐藏窗口截图。隐藏/最小化、跨屏或身份变化明确拒绝。
3. 默认将长边缩至 1024，再只编码一次 PNG；小图不放大，不改系统分辨率。
4. 复查身份/几何，确认清理状态，返回图片及单独时间/质量/反馈说明。

源 DIB 至多 16 MP；`max_edge` 64–2048；PNG 至多 4 MiB；base64 至多
5,592,408 B；原生视觉元数据至多 32 KiB。这些是各层独立限制。
缩小只能控制结果图成本，不能假装没有源图内存开销。

## Pointer 的两种截图方式

- `feedback: "exclude"`（默认）：使用计数式 ticket/RAII 临时隐藏 agent 反馈，
  像素复制完就释放。首次启动、嵌套、旧渲染确认、取消/失败均要处理；
  资源或隐藏状态不确定时不返回成功图。
- `feedback: "include"`：只是**不主动排除反馈**。不清除别人持有的 suspension，
  不保证指针在裁剪范围内，也不把状态计数器当作“图片中确实看到了指针”。
  是否出现仍要看实际图，单帧不能证明连续跟随/FPS。

不缓存为 frame/artifact/view/reference，不刷新旧引用的期限，不改 selected
window 或 foreground anchor。它仍经真实 native worker、截止/取消和设备上下文
调度；不是借 cached_read 绕过。一次调用是一张非原子图，不是视频流，也不
承诺同一 context 的输入执行与截图可并发；不同上下文的资源边界仍适用。

## 调用

```js
const visual = await state.window.captureVisual({maxEdge: 1024, feedback: 'include'});
emit(visual.data);
for (const item of visual.media) emitImage(item);
// 需要输入时，另行 snapshot/observe；不得把 visual_id 当 frame。
```

新工具要由真实 Host MCP 进程加载目录。只替换磁盘二进制或重置 JS worker，
不等于旧父 MCP 已加载新工具；不能用 shell 客户端冒充这条验收链。
实现/离线回归、Host 目录、真实截图/Pointer 和整体性能分别记账。
