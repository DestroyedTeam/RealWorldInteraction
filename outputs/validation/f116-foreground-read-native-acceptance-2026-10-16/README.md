# F116：新版前台读取策略的实机兼容验收

仅操作 BetterDisplay 虚拟屏上的专用 AppKit 程序和独立 Chrome 155.0.8059.84 配置，使用本会话加载的 RWI 工具。

1. 原生窗口后台初态、Chrome 前台初态：四个文字框均为空，无文字、鼠标或键盘替代输入。
2. 首次显式切到原生窗口；独立前台完整身份相符。
3. 首次显式切回 Chrome；独立前台完整身份相符。两次请求均为 AXRaise=1、AXFrontmost=1。
4. 后台原生窗口仍可被观察。新快照 Close window/AXPress 各关闭一次；独立发现窗口消失，绑定分离，四项持久操作终止且资源释放。关闭 Chrome 窗口后仍有独立主进程，经 UID、完整命令、原生出生时间及 ps 生命周期复核后仅终止该进程，原有前台完整身份恢复。原生程序退出0，Chrome 监督器退出0，浏览器实际退出码未知。

**限制：两次切窗均为成功快路径，没有出现 -25204，也没有进入补读分支。因此没有验到真实瞬时错误恢复，不能覆盖或抹去 F95/F99/F105 的失败。** 新策略已通过12项边界测试及冻结策略 B/M/R 对照，但仍需独立原生延迟/失败用例。

全量 Rust 测试1375通过、11忽略；fmt、Clippy全目标全特性-D warnings、架构检查均退出0。工具墙钟耗时只属于本轮两项调用，不代表性能优势。未要求人工配合；未操作游戏、原有文档；未改权限、Host、MCP或密钥。

## 时间顺序截图（原始 PNG 字节）

1. [01-08-native-before](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f116-foreground-read-native-acceptance-2026-10-16/chronological/01-08-native-before.png) — 1320×1104。
2. [02-09-Chrome-before](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f116-foreground-read-native-acceptance-2026-10-16/chronological/02-09-Chrome-before.png) — 2000×1800。
3. [03-14-Chrome-after-first-foreground](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f116-foreground-read-native-acceptance-2026-10-16/chronological/03-14-Chrome-after-first-foreground.png) — 2000×1800。
4. [04-17-Native-before-close](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f116-foreground-read-native-acceptance-2026-10-16/chronological/04-17-Native-before-close.png) — 1320×1104。
