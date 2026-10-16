# 2026-10-03 Codex 重启后 RWI 与 V14 字号探针

## 重启后的工具与设备状态

- 本任务工具目录已刷新：加载 38 个 `computer.*` RWI 工具；当前目录和 Skill 列表均无 Jev 项。无需再为加载 MCP 工具重启 Codex。
- `computer.targets` 返回 `mac-host` available/healthy，平台 macOS，Agent PID 976；Accessibility、Listen Event、Post Event、Screen Capture 均为 true，`input_domain_fenced=false`。`win11-vm` 单独出现连接超时，本次没有访问它。
- 新 discover 定位 Paint S PID 724 / `cgwindow:4228`，标题 `paint-v14-card-cleanup-20260919`，位于 BetterDisplay 虚拟显示器 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`。绑定为只读起点，没有激活或移动窗口。
- 一次完整 snapshot 用时 821 ms（capture 197 ms、capture 后 AX metadata 247 ms），原图 2440×1054，预览 2048×884；列出 57 个可用控件，AX 遍历描述 80 节点，但 41 次子节点读取失败，coverage 不完整。后续 snapshot 样本 434–734 ms。结果只能说明该 Paint 窗口的当前样本，不能外推所有应用。

## 字号试验

1. 当前工作源和 10 月 2 日 219 对象快照哈希相同：`2fc9d4cd8b2c0deccd758a0efce9f113425843e8f6fdfdd9b1245af29f685e94`。RWI 从窗口预览选中第二行标题 Text 层，确认字号控件为 17。
2. 字体族下拉没有出现在可访问控件列表。对其图像点按被 RWI 以 `local_pixel_surface_changed` 在派发前拒绝，原因是 `local_pixel_canvas_unproven`；`native_dispatch_entered=false`、未发输入、围栏未触发，未用其他路径重试。
3. 字号 17→16 是可见、可逆的字号属性试验。首次 `type_text("16")` 追加成 `1716`，AX 读回后通过精确字号框 `cmd+a` 并再次输入 `16` 修正；按 Return 后 AX 读回 16。Text 对象尺寸从 124×20 变为 118×18。之后切回 Rectangle 工具清除选择叠层，再做原像素核对。
4. 以同一参考列表 `reference-list.png`、219 阶段画布裁片和新的 RWI 原像素列表裁片比较第二行标题 ROI `[130,190,277,224]`：本次重新计算的 MSE 为 857.35→591.59（下降约 31.0%）。同一列表裁片中，ROI 外 679 行未发现变化；最后一行是 680 高裁片与 679 高画布边界的差异。由于本次复算的绝对值与 10 月 2 日记录的 1582.91 不一致，只把本次配对结果用于判断字号 16 优于 17，不合并两轮绝对指标，后续需统一量化脚本。
5. 实际 RWI 原像素列表裁图见 [字号 16 当前画面](../../outputs/validation/2026-10-03-v14-font-size-probe/01-after-16-native-list.png)。文件 SHA256 `10f5b05b67be95aa48f53db64f8d892d843d83581b2667e4efb7f2cb4f8eec1a`。

## 保存状态与边界

- 字号 16 目前只在 Paint S 内存文档中，尚未写入源文件。`cmd+s` 因 Paint S 不是当前 focused window 而被 `keyboard_target_not_focused` 拒绝，`native_dispatch_entered=false`；未重试。原 `.paint` 工作源仍为 219 对象/原哈希，旧快照未覆盖。
- 请先把 Paint S 窗口短暂切到前台；回到本任务后即可用新 mutation ID 保存并核验保留 219 对象的字号 16 阶段源。字体族差异仍待 RWI 提供可验证的语义入口。
- 本次没有向游戏、浏览器或其他应用发送输入，没有提交或推送代码。
