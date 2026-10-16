# Windows 系统弹窗、保存、重开及 Explorer 回归

## 结论与根因

不是用户未授权，也不是 Paint 白名单。2026-09-13 用户明确允许 Windows
桌面里的普通系统弹窗、保存/打开、资源管理器及其他应用。

1. **非激活菜单误拦。** Paint `PopupHost / hwnd:10105d8` 的原生 owner 是
   `hwnd:2e0636`。菜单可见、启用，前台却仍是主窗口。旧实现把菜单报告为
   `blocked=true / mutation_ready=false`。本轮在旧部署实际复现，未向被标记
   blocked 的菜单发送输入。
2. **键盘焦点误导。** 主窗口观察同时把 pane 和 Pencil 标为 focused，输入端
   则正确要求 `GetFocusedElement()` 的精确 runtime ID。现在优先观察实际焦点，
   不再把每个 provider 的缓存焦点标记提升为键盘目标。
3. **返回 owner 授权过窄。** 首次独立选择 Open 对话框后，返回主窗口仍沿用
   对话框作为临时 grant 根，现场报 `window_scope_mismatch`，输入未派发。
   已改为桌面授权下每次验证过的 selection 都重新收窄到选中窗口；旧的
   root-only grant 不放宽。
4. **系统快捷键硬限制。** 原 parser 禁止 Win / Alt+Tab / Ctrl+Escape，并非
   Windows 或 MCP 安装失败。现在桌面 policy 允许普通 shell 快捷键，旧的
   窗口限定模式保留原限制；不把 secure attention 当作 SendInput 能力。

原独立绘图任务确实已经 GUI 保存成功，失败的是部分前置菜单和最终重开节点。
本报告是后续修复回归，不回写原任务的历史结果。

## 实际入口及应用证据

全部 GUI 输入/观察由当前 Host 加载的 `computer.js → computer.*` 原生 MCP
发起，没有 Python/Node shell MCP consumer、SendKeys 或图片导入替代 GUI。
SSH 只用于 Rust 构建物部署及文件只读核对，是独立互补证据。

| 节点 | 现场结果 |
|---|---|
| 非前台 owner 菜单 | 新 agent 报 ready=true、blocked=false；精确焦点 New 上的 Escape 成功，随后 native discovery 证实菜单已隐藏 |
| File → Open | 语义点击菜单 Open 成功，原生 `#32770 / Open` 前台对话框可观察、File name 为唯一权威焦点 |
| 对话框取消 | 精确 filename 焦点上 Escape 成功；未修改画布 |
| Ctrl+Shift+S | 从主窗口打开 Save as；明确选择 PNG，而非默认的 Paint Project 格式 |
| GUI 另存副本 | 输入新的文件名并点击 Save；未覆盖原来的 `rcu-nemessix-native-20260913-0230.png` |
| 返回 owner | 从独立 initial 选中的 Save as 返回原生 owner，成功且 ready=true |
| Ctrl+O → 文件名 → Enter | 已真实重开新副本；独立窗口标题 predicate 两次匹配；画布 tile 哈希前后一致 |
| Win+E | 真实打开 `Home - File Explorer / PID 6016 / hwnd:2f051c`，在同一 MCP target 下 initial 选择，无新任务/连接/应用授权 |
| Explorer 操作 | 点击导航树进入 RCU-Codex-Benchmark，截图看到新 PNG；Alt+F4 关闭该测试窗口，native discovery 确认该 HWND 不在完整匹配结果中 |
| 负面焦点检查 | Explorer 前台时，后台 Paint 仍为 blocked=true / mutation_ready=false，没有对后台 Paint 发送输入 |

保存文件：
`C:\Users\vincentadamnemessis\Desktop\RCU-Codex-Benchmark\rcu-dialog-roundtrip-20260913-r2.png`

- SSH 只读确认：12,676 bytes，PNG 签名 `89-50-4E-47-0D-0A-1A-0A`。
- 文件 SHA-256：`d155aa4f352970d2f8ad3ea6823e9e7b763c2f7ddc4a78f1a72165456863a26b`。
- 重开前/后原生 512×512 画布 tile SHA-256：
  `ab04a812882dc0a16fa93ab02d78cc8aaf172627a3d7d5b66f3379503fa7ae75`。
- tile 与文件编码哈希是不同对象。当前画布在测试前已有细线，本轮完整保留，
  没有按旧任务截图擅自修图。

## 诚实记录阻断及范围

首个旧部署 File 点击后立即 discover 尚未看到菜单，caller 提前结束该 cell；
稍后仅作新观察即发现菜单，未重发 File 点击。另一处是上面明确记录的
return-to-owner grant 拒绝，修复后再做独立回归。原生输入回执审计记录的
19 次输入均 accepted，但这不抹去这两次 cell 阻断，也不代表整个产品完成。

普通系统弹窗已测试，不将其外推到所有软件、secure desktop/UAC、跨显示器、
Mac/Linux/移动端。没有新增应用白名单、后台输入、自动抢焦点或不确定动作重放。
没有本轮新的绘图效率、端到端模型思考时间或 Pointer 显示延迟测量。

## 工程及可回滚部署

- 行为提交：`79ccaef`；纯键盘 parser 模块搬迁：`a91dc75`，函数体除局部可见性
  和模块位置外不变。架构 ratchet 通过不等同架构全部完成。
- 回归主部署 agent SHA：
  `f644c07db4ff64fa9eb429fb88c5660a37965406b66e2bde9b5e21ca72a5f034`。
- 模块搬迁后的最终部署 SHA：
  `2aa4b454d8ff22273d579607fc006b8cc78f34126208e49534bb62f50a882b26`，
  PID 12996 / Session 1。重新通过 Host MCP 的 Ctrl+O、独立选择 Open、Escape
  及 return-to-owner；最终画布 tile 哈希仍相同，ready=true / fenced=false。
- 配置 SHA 保持：
  `b6dc9b892b1ae39f681f88cb5269b0bc8d163a50e046bfc64e533026318d29b7`。
- 使用原有 `RealComputerUse-Interactive-Agent` 任务，保留 Session 1 无控制台
  agent。原配置、启动任务及网络策略不变，旧二进制分别保留在 rollback 目录。
- 操作日志保留于 `work/dialog-focus-20260913/`，原始截图、二进制、临时运行状态
  不提交 Git。通用设计与 API 依据见 [Windows dialog input](../architecture/windows-dialog-input.md)。

最终回归：`cargo fmt --all -- --check`、workspace 默认测试 290 passed / 3 ignored、
all-features 377 passed / 4 ignored、JavaScript 18 passed、workspace all-targets /
all-features clippy `-D warnings`、Windows ARM64 cross-clippy 和 release build 通过。
架构 ratchet 无新增违规；`architecture_complete=false` 的既有债务仍单独保留。
