# 通用 headless 架构迁移：实际进展与证据

日期：2026-09-12。**不是完整架构/全功能验收通过声明。** ADR 0002 是整套产品必须遵循的架构，不能把交付拆分理解成暂时绕过约束。

## 已实现并测试的范围

- 默认 `rcu-mcp` 使用独立设备 profile；零设备、离线设备都不使工具目录退化为诊断模式。
- 默认 `rcu-native-link` 依赖图不包含 Windows/macOS driver、MCP SDK 或业务编排；默认 adapter 图也没有平台 driver。
- 新 `rcu-agent` 不读取业务 manifest、预期文本、Stage 或 oracle。认证主体、上下文、共享桌面租约、持久化操作、防重放与不确定资源域分别处理。
- 两设备相同 operation ID 明确返回歧义，支持显式 device ID。Host 取消、EOF/丢弃传播限定设备和上下文的取消请求；清理使用一个并行总预算，不等于强行停止 native provider。
- 每个主体独立保存 Core 回执，但共用设备侧 lease root。资源恢复保留全部旧操作，仅对经独立检查的输入域解除限制。
- 已退役本次被否定的 `ProcessCleanupSeed`、`process_goal`、专用进程清理 oracle/CLI 扩展；只留下不可执行的历史补丁证据。旧的历史 Notepad harness 仍在 `legacy-workflow` 下，物理迁出尚未完成。

## 真实 Host 与 Windows 证据

实际 Codex `app-server` 的新临时测试任务加载本项目配置，发现 **20 个正式工具**和更新后的 Skill，描述不含 `DIAGNOSTIC ONLY`。项目注册保持 enabled/required，指向稳定 `.rcu/bin/rcu-mcp --profile .rcu/profile.json`，不再指向 `work/.../admin5`。这不是全局插件安装或新模型自主运行的证明。

Windows agent 以 GUI subsystem 编译，实际 PID 6820，Session 1，MainWindowHandle 为 0；独立监听 `10.211.55.3:45128`，不需要启动测试应用。安装失败过一次：operator 临时 bundle 文件的 ACL 未保护；补齐临时文件保护后通过 Rust importer 安装。产品没有改成忽略 ACL。

用户明确批准指定通信后，新增 **仅此 agent、TCP、Mac `10.211.55.2` → Windows `10.211.55.3:45128`** 的规则，禁用此 agent 原先的宽泛 TCP Allow。自动规则在处理期间从 Block 变为 Allow，不归因于首次失败的脚本。UDP 与全局 profile 未被本任务修改。

经实际 Codex Host → 正式 adapter → mTLS device RPC → Rust Windows driver：

1. 发现并绑定现有任务管理器，无业务文件或应用启动。
2. 在 `document_state = null` 的非文档界面，通过 `act_checked` 点击 Performance。
3. 新 UIA 读取和新 PNG 显示 Performance/CPU 页面，不用 input ACK 证明效果。
4. 再经 checked click 返回 Processes，新读取/图像确认恢复；原有 `rcu` 搜索内容保留。
5. 共 2 次 SendInput、6 个输入事件；两次租约均释放，操作 terminal/resources_released 为 true。没有结束任何进程。
6. `computer.verify` 两次独立 Win32 标题采样匹配 Task Manager。这里 legacy `goal_verified` 仅指该原生谓词，不是整项业务完成。
7. 关闭 Host 后，再开新 Host，查询旧 operation ID 得到不可重放回执；同一个 agent 实例继续服务，并生成新的上下文 target。

这覆盖 **H + 有限 N**。没有宣称本轮验证键盘、全部取消/崩溃场景、所有平台或自主模型 M 层。安装用 operator 脚本没有实现截图、输入或运行时 RPC 桥接；真实 GUI 操作均由 Rust MCP/native driver 执行。

## 回归与未完成项

- 默认 workspace：192 passed，1 ignored。
- 全 feature 离线回归：283 passed，2 ignored；显式提供独立预构建的 legacy fixture worker，未把该 worker 部署为生产 adapter。
- fmt、host all-target/all-feature Clippy、Windows ARM64 agent Clippy、架构 ratchet、Skill/plugin 文件格式检查均通过。
- 独立复审尚待 Control Center；自测不是它的最终裁决。

还需完成：通用应用启动策略、完整 ordinary/admin 与窗口关闭/焦点变化/取消/崩溃的原生矩阵、compact 输出与原始证据 MCP 取回、legacy harness 物理迁移、其他平台驱动接入和实测、实际全局插件安装与自主模型验收。当前观察输出仍很大，**没有效率或模型理解改善的验收结论**。

结构化证据索引：[JSON](2026-09-12-service-migration-progress.json)。原始 RPC、私密配置、二进制和 PNG 留在被忽略的本地工作目录，不提交。
