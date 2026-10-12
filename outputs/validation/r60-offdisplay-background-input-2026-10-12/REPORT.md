# R60 · 偏出屏幕的窗口局部后台输入

当前：RWI macOS / 同源实机后台滚动与前台越界负例已通过 / 后台悬停、跨显示器拖拽等剩余项继续保留。

## 按真实执行顺序

1. **原130保留失败**：窗口 `[3225,504,660,552]` 右侧偏出虚拟屏165点。原滚轮请求在 `hit_rect` 校验前置拒绝，输入0、scroll.events0。该窗口后来按480秒生命周期关闭（expiry1 / exit0），不是显式关闭成功。原请求没有重放。
2. **第一候选不是成功修复**：同一131窗口上的实际点 `[3745,844]`，命中范围回读为 `[1800,0,1920,30]`，`same_native_window=false`；新增数值诊断明确了不相容位置，但该候选仍拒绝/输入0。不能根据这个矩形推断返回对象的AXRole，或把“夹紧”当已证实原因。
3. **第二候选修正 `point_element`**：对于仍与非旋转有效屏相交、且实际点位于窗口但不在任何有效屏上的情况，先从精确AX窗口的有界原生子树解析包含该点的对象；不借用应用级命中、不移动源窗口、不夹紧坐标。不授予树选窗口“正向应用命中”的空canvas证明；原native scope、对象、分类、几何、modal、lease及无重放检查仍执行。
4. **同一131实机成功**：新的独立滚轮请求发送一次 `delta_y=-240`。输入前后 `app_active=false/window_key=false/frontmost_pid=95007`；scroll.events **0→1**，clip_origin **[0,0]→[0,240]**。实际截图的首行 **00→08**。工具elapsed_ms385（resolve5/claim1/act372/release4）；这是该适配器调用墙钟时间，不是CPU时间或与其他工具的性能排名。应用效应来自独立原生状态/像素，不来自 `dispatch=returned`。
5. **前台边界负例**：对自有131明确执行一次AXRaise/AXFrontmost，实际95007→98005；随后对同一点请求 `foreground_system` move。实际返回 `foreground_point_unmapped/rejected_before_dispatch/native_dispatch_entered=false`，没有把窗口局部坐标转成越界系统光标事件，也没有换到后台路径。scroll仍1、drag0、hover0。
6. **清理**：自有131/132分别一次明确AXClose，closed/expiry0/进程exit0，两个固定绑定已detach。7个原durable请求全部terminal、released、无pending/无release_uncertain、禁止replay。并未操作其他前台应用的内容。

## 图像（原始PNG，不加滤镜、不重绘）

### 01 原130输入前

![01 原130输入前](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r60-offdisplay-background-input-2026-10-12/01-baseline-wheel-refused.png)

### 02 第一候选131，仍为第00行

![02 第一候选131，仍为第00行](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r60-offdisplay-background-input-2026-10-12/02-first-candidate-still-refused.png)

### 03 同一131修正后输入前

![03 同一131修正后输入前](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r60-offdisplay-background-input-2026-10-12/03-before-window-local-wheel.png)

### 04 后台一次滚轮后，第08行

![04 后台一次滚轮后，第08行](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r60-offdisplay-background-input-2026-10-12/04-after-background-wheel.png)

## 部署、回归及范围

- 两个候选的实际部署事件均保留；最终Agent PID2772 / instance89dec9f4-e1be-4e50-b800-6d28db3bae5c。稳定本地证书和指定要求未变，四项权限仍true；CA、profile、配置及安装MCP哈希未变。此轮无需用户授权或重启Codex。
- 当前最终源的完整11门exit0：workspace768 passed/0 failed/11 ignored/42 suites；Receiver66；Live UI7；fmt check、全targets/features Clippy-Dwarnings、release及架构门通过。
- 源配置使用相同命令/输入：BASELINE1、MODIFIED0、独立副本ROLLBACK1、REAPPLIED0；精确恢复原hash。源copy回滚不是生产Agent回滚、也不是原生输入回放。最初具体cargo回归真实101→0，日志保留。
- Live既有改造继续保留：`panel_size` 宽200–320点、按画面比例布局；隐藏用eye.slash，关闭用xmark，原生无障碍标签/tooltip完整。R48已观察到320×303及真实Host任务切换/关闭；本轮不伪造Host presence、不声称重新做过用户可见Live验收。OpenAI公开说明支持macOS画中画预览、打开/移动预览，但没有给出本页可依赖的精确尺寸或按钮样式。[官方说明](https://learn.chatgpt.com/use-cases/use-your-computer-with-codex)
- Apple将应用级命中接口定义为屏幕坐标查询；本次“越界点却返回不包含该点的顶部矩形”来自实际回读，而非文档保证或普遍客户端行为。[AXUIElementCopyElementAtPosition](https://developer.apple.com/documentation/applicationservices/1462077-axuielementcopyelementatposition)
- **未完成全计划**：整窗离屏、其他Space/最小化、混合DPI、跨显示器drag、泛化后台tracking/tooltip、foreign-provider菜单、跨应用DnD仍保留。当前输入效果仅对明确测试的同一原生fixture及这一次背景滚轮成立。当前改动尚未提交推送；既有远程CI不能充当本候选CI。
