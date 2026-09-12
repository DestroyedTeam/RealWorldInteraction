# A1 菱形 Pointer：Windows 可回滚开发版

## 范围

用户已选 A1 菱形，并明确批准“先推进开发版，不声称正式 UIOS 验收”。
本次不继续 UI 选型、不伪造签名链；UI Quality OS 只作为设计依据。
原始参数/SVG/CSS 存在 `reference/pointer-a1-20260913/`。

Rust 将纯动效状态、材质、原生偏好/GDI 字形及 HWND 生命周期分开。
实现层叠光影、按压反馈、有限的速度/反向形变、空闲常驻、分数 DPI 和标签避边。
热点保持真实物理位置，不平滑预测输入；无应用名、业务流程或 Python 后端。

源码复核发现的两项问题已修复并加测试：关闭动画时平衡点击仍有短暂静态
接触提示；空闲 DPI 改变刷新材质但不移动热点。HC 热点不再保留蓝色阴影。
现场改变系统 DPI/主题/Reduced Motion 本次未做，不把纯模型测试当成现场结果。

## 部署

- Windows ARM64 SHA256：
  `fa5827f56f459a6d9d709ad46587ea6e3b60ada564477320b3f7b7970a087496`，
  4545536 bytes；GUI 子系统，复用既有交互启动任务。
- 停止前逐项检查旧 PID、路径、Session、启动时间、旧/候选文件 SHA、配置 SHA
  和启动任务目标；只替换 RCU Agent，并保留独立 `rollback-a1-diamond-v1`。
- 新进程 PID 12116，Session 1，实例 `78049667-151c-4212-a471-ce6ccaeb7b25`。
  配置及任务定义未变；未修改 SSH、防火墙、凭据或其他应用。
- SSH 仅用于部署/进程诊断。下列 GUI 操作均来自当前 Host 已加载的原生
  `computer.*` / `computer.js`，没有 Shell 启动的 MCP 验收替身。

## 实际 Host MCP 检查

1. **零输入启动。** 新实例 available/healthy、无 fence；设备实时反馈显示
   `device_native_a1_diamond`、initialized_from_service/visible=true、idle、
   updates_requested=0、render_submissions=1。DPI 192，实际 GDI Segoe UI 字形，
   Windows animation=true、light theme。启动位置读取真实鼠标，没有发送输入。
2. **真实应用。** 新鲜发现/绑定用户分配的 Paint，PID 2024 / hwnd:2e0636，
   前台、可用；1024px 映射预览显示空白画布。未修改显示分辨率。
3. **一次原生点击。** 新鲜 Select 元素，稳定请求
   `a1-diamond-select-smoke-20260913-r1`；checked action 接收、终止并释放，
   withInput lease 已释放。此 ACK 本身不计画图/业务完成。
4. **原生反馈状态。** 点击后 anchor=[470,436] / DPI 192；采到 pressure≈0.993、
   animating=true，随后 idle/pressure=0、visible=true，位置保持，无 error。
   原生枚举的反馈 HWND visible=true、enabled=false、foreground=false。
   这是状态采样，不是录像或用户对新 Windows 外观的再次确认。
5. **独立效果复查。** `computer.verify` 对 Paint 的 focused_window 连续两样本
   匹配；它只证明此窗口仍前台，不证明已绘图。点击后两次完整 grounding PNG
   SHA 均为 `8fef5db94425d7c0ef8f9a68e776a143841184f5457674ab5463a70dc2e2d40d`。
   首次点击前 SHA 不同，因此不声称全部帧字节相同。画布保持空白，未保存/清空。
6. **连续轨迹入口：明确未通过。** 对同一 Select 控件内 400 ms / 6px 的已知
   有界轨迹，仅尝试请求 `a1-diamond-bounded-select-drag-20260913-r1` 一次。
   当前旧 Host 适配器在参数解析处报告 `unknown variant drag`，没有设备执行
   引用、没有原生输入，lease 正常释放；无重放、无降级为像素输入。
   后续原生观察/verify 正常，设备 updates_requested 仍为 1。
   这是当前运行适配器与磁盘新版/设备能力不匹配，不是 Paint 阻塞服务。

## 性能证据和未通过项

- 点击 JS cell 2139.37 ms：observe 640.09、claim 5.37、checked action 1439.51、
  release 52.52 ms；不是纯输入耗时，也不是整轮任务耗时。
- 单次设备 raster 样本约 4.84–5.20 ms，记录到一次峰值 72.66 ms。该计时
  不含材质缓存建立、GDI/合成器/网络/模型间隔，不能据此宣布 60 FPS 或丝滑达标。
- 离屏 Rust raster study 与实际 GDI 字形分别记录。近似 blur 不等于浏览器
  像素级复刻；正式 UIOS 签名验收仍未完成且不阻塞此次开发版。
- 连续轨迹需要刷新真正的 Host 适配器后从同一原生入口复验。多笔编排、反馈
  入图的只读观察通道、动态延迟、主题/DPI 现场变化及 Computer Use 等价仍待验。

## 回归和后续

- fmt check、默认 workspace locked/offline：272 passed / 3 ignored，34 suites。
- workspace all-targets/all-features Clippy `-D warnings` 通过。
- Windows ARM64 release build 与同目标 Clippy 通过。
- all-features workspace locked/offline：359 passed / 4 ignored，38 suites；使用
  显式预构建的隔离 fixture worker，不是 native/Host 绘图验收。

本机原始工具回执：`work/a1-native-20260913/live-mcp-evidence.json`；同目录有
部署前后哈希、构建和测试日志。原始运行状态、截图与二进制不提交 Git。

接下来回到功能主线：先让新 Host 入口明确支持 typed drag，再做真实 Paint
连续笔画、取消/焦点变化检查；并分离只读可视观察与带 UIA 的输入 grounding。
不再重复 UI 方案选择，不因开发版运行成功宣布整体可替代 Computer Use。
