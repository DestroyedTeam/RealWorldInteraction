# Paint S 弹窗放置与真实回读（2026-09-25）

## 范围与实际顺序

仅用磁盘源的临时副本 `work/paint-v14-semantic-dialog-test-20260925.paint` 作 GUI 验收，所有 GUI 点击、拖拽、绑定、放置均由当前 Codex 任务的 RWI MCP 发出。BetterDisplay 虚拟屏 UUID 为 `13E4E9B0-47F2-4043-9CDC-878D5441BD41`，当次 bounds `[1512,0,1920,1080]`。正式 V14 编辑窗口 `cgwindow:2189` 未作为本轮输入目标。以下为真实 Host 回执与**独立再次发现**，不是离线 fixture 代替实机；RWI `dispatch=returned` 不等于业务目标成功。

1. 重新打开测试副本，RWI 精确绑定后将文档从内建屏 `[204,190,1220,527]` 移至虚拟屏 `[1536,24,1220,527]`。按 `Resize canvas and image` 的 `AXPress` 返回 `AXError -25204` / `dispatch=uncertain`；没有重放。独立发现证实普通 `Resize` 弹窗 `cgwindow:6191` 已在内建屏 `[596,308,379,384]`。只读 AX 诊断发现弹窗 `AXModal=true`、`AXPosition` 可写；Paint S 的 `AXFocusedWindow` 却仍是文档 `cgwindow:6177`，应用级中心命中属于弹窗自身 ID `6191`。
2. RWI 以精确窗口号、PID、epoch 绑定 `Resize`，调用 `computer.place_window`，原生唯一一次 `AXPosition` 后返回 `[1536,24,379,384]`；独立发现确认它确实在虚拟屏。旧版快照仍报 `blocked=true`、全部按钮 `occluded=true`，尽管 RWI 单窗图像能清晰显示 `Cancel`/`OK`。这证明是模态判断缺口，不是截图糊或窗口不存在。
3. 修订仅在**所选 AXModal 弹窗自身的应用级 hit-test、AXWindow 对象、PID 和 CGWindowID 完全一致**时，允许它在主文档仍为 AXFocusedWindow 的情况下作为可操作面。另一个聚焦模态、命中其他窗口或命中未知仍被阻断。稳定签名部署后，相同的实际 `Resize` 弹窗快照变为 `blocked=false`、`mutation_ready=true`；RWI 对 `Cancel` 的 `AXPress` 返回，独立发现确认弹窗消失。测试副本随后由 RWI 点击精确文档关闭，正式源仍在。
4. 再次打开测试副本，只在副本里选中企鹅头像并拖动尺寸手柄；本次没有再弹出先前的提示框，因此**不能声称已把那个提示框搬到虚拟屏**。关闭已修改副本后，虚拟屏出现保存 Sheet：逻辑 `cgwindow:6506` / `[2016,158,260,258]`，独立可视 `cgwindow:6507` / `[1941,83,410,408]`。只读诊断显示逻辑 Sheet 的 `AXPosition` 报可写。
5. 为窗口级放置开放无像素 `Locator::Window`（只产生不可用于输入的 placement reference，不伪造像素映射），原生端重新验证该逻辑 Sheet 的自身 CG ID/PID/焦点/几何和 AXPosition 可写。首次实机 `AXPosition` **返回成功但 Sheet 位置未动**；旧 `placement_confirmed` 因只检查“仍在目标屏”而误报 true。后续修复要求同目标屏、达到本次请求的精确起点（≤1pt 容差）且连续两次读回一致；新版重试是**新的明确诊断请求**，没有重放旧 request ID。结果为 `placement_confirmed=false`、`requested_origin=[1536,24]`、读回仍 `[2016,158,260,258]`，15 次/975ms。独立发现逻辑 ID6506 与可视 ID6507 的位置均不变，正确揭示应用忽略这次写入。RWI 随后对逻辑 Sheet 明确 `AXPress Don’t Save`，独立发现 Sheet 和测试副本均关闭；正式源仍在。

## 回归、部署、界限

- 新增回归覆盖无像素 Window 引用可用于放置而不能点击/像素定位、未聚焦但精确命中的模态弹窗、以及“仍在目标屏但 AX 忽略请求坐标”不得确认。最终 `cargo fmt --all -- --check`、`cargo test --workspace --locked --offline` **516 passed / 10 ignored**、全 targets/features `clippy -D warnings`、`git diff --check` 与 Agent release 构建通过。
- 最终已签名安装 Agent SHA-256 `82271f8513f6b540d7d4293e08f6e9df52a3cab38f5d1b812e6ea86916a48f66`；MCP 原安装二进制未更换（SHA-256 `551c37f09232a491244f3bdcfdc6580abfadf014ed4e6c6abcd7e54f136294e4`）。本地证书 designated requirement、Agent 配置和 TLS client profile 均未改变；Agent 重启后 mac-host healthy，四项 TCC 均 true。
- macOS Skill 已更新“模态但焦点仍在主文档”“无像素窗口级放置不赋予输入像素权威”“AXPosition 返回不等于实际搬动”三项决策规则；本机 Codex Plugin `0.2.0+codex.20260925071534` 已验证并安装。当前任务仍可继续使用原工具 schema；新 Skill 内容由后续 Host 任务/重载拾取。
- 正式工作源、正式输出快照及临时副本磁盘文件均为 446815 bytes、SHA-256 `1573f4f74b45a6c13014dd9c0efeb1d2d3a2185f9fa4368edab74c91ba8a9998`；本轮只丢弃测试副本未保存的手柄变更。原 V14 仍未完成，不能把此弹窗测试说成画稿进展。
- 实际 RWI 单窗截图在本任务工具记录里依次展示了：虚拟屏主文档、移动到虚拟屏但误报遮挡的 `Resize`、修订后可操作的同一弹窗，以及源参考图；移动前的内建屏位置由原生发现和 AX 诊断记录，而非截图推断。先前已归档的实际 Paint 截图见 [动画选框验收](2026-09-25-animated-canvas-and-semantic-sheets.md) 与 `outputs/validation/2026-09-25-animated-canvas/`；本轮弹窗没有另存含用户桌面信息的全屏截图。
- 逻辑 Sheet 的 AXPosition “可写”不代表 Paint S 执行了移动；其**可视渲染面也没有跟随**。普通同 ID `Resize` 弹窗实测可移，先前不同 ID 的提示框尚未复测。跨进程系统面板自动关联、长输入期间 Live 争用、V14 继续绘制均仍待办。
