# Windows Pointer：服务常驻与原生 MCP 验证

本记录接续 `2026-09-12-pointer-feedback-initial.md`，保留初版短时反馈的历史，
不把初版结果改写为常驻版结果。用户明确要求：Agent 运行时，即使没有任务也
保持在上次位置；用户随后确认“能看到是能看到”，并指出动效缺失、样式需要
UI Quality OS 重新设计，且必须区别于 Windows 自身鼠标。

## 行为与部署

- 反馈由 serving agent 启动，不等待 MCP 查询、输入或业务窗口；任务结束只
  回到 idle，不销毁标记。启动读取当前 OS 指针位置，不移动真实鼠标。
- 最后位置保存在 agent 内存。重启读取当时的系统指针位置，尚未实现落盘恢复。
- 800 ms 是活动高亮的时限，不再是标记寿命。前台改变或用户移动鼠标不会
  让 idle 标记追着用户走，也不会把用户鼠标拉回。
- 新增通用 backend runtime_status 透传，避免把网关缓存的 capabilities 当成
  实时状态。实时读 `devices[].state.runtime_status.pointer_feedback`。
- Windows ARM64 agent PID 1500，Session 1；实例
  `f6cf606a-51a7-4d27-94b9-c871fde92ff3`。
- 运行二进制 SHA256：
  `1d94617304bbf1933c0c155108a3d9cdd67cecb23b11c8aefc47e6126b37fe8e`，
  4518400 bytes。复用既有交互启动任务；只替换 agent，保留 rollback。
- 配置 SHA256 保持
  `b6dc9b892b1ae39f681f88cb5269b0bc8d163a50e046bfc64e533026318d29b7`。
  未修改凭据、SSH、防火墙或任务定义，未关闭 Paint/Codex。

## 实际 Host 原生 MCP 验证

GUI 检查全部通过当前任务实际加载的 `computer.*` 工具完成；SSH 只承担构建
产物部署及进程诊断，不是 GUI 代理。用户分配的前台窗口为
`Untitled - Paint` / PID 2024 / `hwnd:2e0636`，空白 512×512 画布。

1. **零输入启动：** `initialized_from_service=true`、`phase=idle`、
   `visible=true`、anchor `[470,436]` / scale 2、`updates_requested=0`、
   `render_submissions=1`，error=null。
2. **无任务常驻：** 两次设备原生采样时间为 1789223012287 与 1789223118580，
   相差 106293 ms。位置、idle/visible 状态及 render_submissions=1 均未变，
   此时尚未发送本版本任何输入。该区间包含工具间规划间隔和一次显式等待，
   不是声称连续录像或休眠了 106 秒。
3. **穿透检查：** 原生发现反馈窗口 `hwnd:1b037c` / PID 1500，visible=true、
   enabled=false、foreground=false；在标记覆盖的 `[470,436]` 位置点击新鲜
   UIA Select 元素。请求 `persistent-pointer-select-smoke-20260912-r1` 返回
   system_accepted=true、terminal=true、resources_released=true，无 fence。
   再读确认 Paint 仍前台；lease 已释放，未重放输入。
4. **点击后回 idle：** 等待 1100 ms 后 visible=true、phase=idle、位置不变，
   updates_requested=1、render_submissions=4、error=null。
5. **grounding 隔离：** Select 点击前后完整 PNG SHA256 均为
   `8fef5db94425d7c0ef8f9a68e776a143841184f5457674ab5463a70dc2e2d40d`。
   后次 capture 的 feedback_suspend_ms=4.082084；这是一次采样，不是延迟分布。
   UIA 可命中，画布仍空白；本轮没有落笔、保存或清空文档。
6. **人工可见确认：** 用户明确回复能看到常驻标记。此确认不包含动效、美观度、
   动态跟随延迟、绘图流畅度或 Computer Use 等价验收；用户反而要求改进这些。

设备 visible/counter 仅为原生状态；人工反馈与机器证据分别记录。
现有 grounding PNG 会短暂隐藏自身反馈以保持内容哈希，不能用于直接展示该
反馈外观。反馈入图的独立可视化捕获仍未实现。

## 回归

- `cargo fmt --all -- --check` 通过。
- 默认 workspace locked/offline：270 passed / 2 ignored，34 suites。
- all-features workspace locked/offline：357 passed / 3 ignored，38 suites。
- workspace all-targets/all-features Clippy `-D warnings` 通过。
- Windows ARM64 cross Clippy 与 release agent 构建通过。
- 原生技能 quick_validate 通过。

原始运行证据：`work/persistent-pointer-20260912/live-mcp-evidence.json`；
同目录保留测试、构建与部署记录。原始截图、二进制和运行身份状态不提交 Git。

## 下一步边界

UI Quality OS 设计新外观和动作动效，与本次服务生命周期修复分开提交。
必须保留常驻、透明点击、不抢焦点、无输入重放及截图隔离。绘图、多笔批处理、
键盘提示、跨平台对齐、动态性能和正式 UIOS 验收均不由上述检查替代。
