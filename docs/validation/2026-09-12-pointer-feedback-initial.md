# RCU Pointer 初版：代码/部署通过，视觉验收待完成

用户要求见 `../product/requirements-and-acceptance.md`；机制与明确限制见
`../architecture/pointer-feedback.md`。本版本没有改应用业务流程，也没有改 MCP
输入 schema。**仍未宣称与 Computer Use 打平。**

## 已完成

- Windows Rust 设备侧透明反馈层：实际指针位置、蓝色光晕、RCU 标识、拖动按下
  环；不调用输入或激活 API。一次原始输入失败/取消后的释放规则保持不变。
- 单槽进度合并、短期过期、同 PID/HWND 前台约束；捕获暂停按 revision 确认。
- 显式隔离反馈与 grounding 图片：只在 GDI 拷贝期间隐藏自己的反馈，防止
  指针变化破坏内容哈希；现有 grounding PNG 不包含该反馈。这是当前限制。
- 5 个新增可移植测试，覆盖 raster/DPI/alpha、按下态、旧确认与嵌套 capture。
- `cargo fmt --all -- --check` 通过。
- 默认 workspace 测试：269 passed / 2 ignored，34 suites。
- all-features workspace：356 passed / 3 ignored，38 suites；使用独立 fixture worker。
- workspace all-targets/all-features Clippy `-D warnings` 通过。
- Windows ARM64 cross Clippy 与 release agent 构建通过。

## 安装与原生入口

Windows 新 agent PID 8024，交互 Session 1；实例
`2b265dd6-d0b4-4016-b975-08535e81e86d`。

运行文件 SHA256：
`98b306f1b18891924acbc228013a5bf7b9e9210e5cc7aa42c29397655957f155`。
旧版 `37c616450be5eaab4d65abf12ce2142391f32ddb2435c19929b8733e1ce70644`
已保留在 agent 安装目录的 `rollback-pointer-v1/rcu-agent.exe`。

更新前核对原进程 PID/路径/Session/创建时间及新旧文件 SHA。仅替换既有 agent，
复用 `RealComputerUse-Interactive-Agent`；配置、凭据、SSH/firewall 未改。
部署后由本任务实际加载的 RCU MCP 查询到新实例、健康状态及 `pointer_feedback`
能力字段；`started=false`，说明只读查询没有偷偷启动可见反馈窗口。

## 尚未通过的原生检查

Paint `hwnd:506a4` / PID 12116 当前不在前台。已询问用户把明确分配的 Paint
放到前台；同时仅尝试一次受权原生 `activate_window`，Windows 返回
`api_accepted=false`、`state=not_foreground`。操作终态且资源释放，lease 已释放；
没有重放、热键绕行或脚本输入。

因此尚未发送本版本测试点击/拖动；屏幕上的真实反馈、UIA 命中不被遮挡、后续
像素 grounding 稳定性与动态延迟均待验证。后续只测试 Paint 工具栏，不修改
用户参考画稿；真正绘制和多笔连贯性要单独分配测试文档。

后续还需：反馈入图的只读捕获通道、去掉只读图片请求中的 UIA 扫描、有界多笔
动作编排、键盘状态反馈及其他平台。初版部署不是这些项目全部完成的证据。

本地原始日志、构建/部署脚本和配置哈希记录保留于
`work/pointer-feedback-20260912/`，不提交运行二进制或凭据。
