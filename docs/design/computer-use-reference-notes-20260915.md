# Computer Use 参考材料调研与 RWI 落点（2026-09-15）

来源（用户提供）：
1. 微信文章《为什么 Codex 操作我的电脑时，我还能继续工作…》(mp.weixin.qq.com/s/3gMB4s_dli1ZjqPpOy3iGQ)
   —— 转述 Bridge 团队对 macOS 后台 computer use 的拆解。
2. 开源实现：github.com/EYHN/kwwk-computer-use-core（Swift，macOS 14+，MIT；
   其 README 注明后台激活与窗口本地事件投递部分源自 trycua/cua 的 cua-driver）。

## 一、文章给出的三层能力（与 RWI 现状对照）

| 文章要点 | RWI 现状 |
|---|---|
| ① AX 读取后台窗口结构（窗口/按钮/输入框/位置/名称/可点性） | ✅ 已实现（observe/scene，bounded AX 树） |
| ② `CGEvent.postToPid` 把点击/按键定向投递到指定进程（不动真实鼠标） | ✅ 已实现（窗口寻址字段 51/58 + 窗口局部坐标 + primer） |
| ③ 让后台窗口"自认为已激活"但不真的切前台（监听焦点切换→发准备激活事件→窗口中心一次无功能点击→阻止系统切前台） | ✅ 同源路线（`activate_background_window` + 交替位置 primer），与 kwwk/cua 的公开做法一致，可交叉验证 |
| ④ 浮动第二光标 = 给用户的视觉反馈（非功能鼠标） | ❌ 缺失（macOS `pointer_feedback=implemented:false`） |
| ⑤ 兼容性局限：Chrome/Electron 后台 AX 不完整、部分按钮不可稳定触发、系统升级可能改变行为 | 部分覆盖（fail-closed + 重试 + sheet/panel/popover 特殊路径），但 Electron 类仍需专门验证 |
| ⑥ 授权与操作记录 | ✅ 租约 + 持久 no-replay 台账；待补的是"面向用户的接管提示/停止入口" |

## 二、kwwk-computer-use-core 的可借鉴设计（README 层）

- **面向 agent 的控件清单**：`state(app:)` 返回节点快照（role/title/index…），既可结构化
  给产品代码，也可格式化成"agent 可读文本"交付给 agent 自行推断用途。
- **次级动作枚举**：`performSecondaryAction(snapshotID:elementIndex:action:)` —— 把元素的
  **可用 AX 动作列表**暴露给 agent（"这个按钮能做什么"），而不是只支持一种动作。
- **会话级"后台激活 + 焦点抑制"状态**：`ComputerUseClient` 持有 session，序列动作共享该状态。
- **输出双形态**：`ComputerUseCommandOutput` 同时给格式化文本 + 结构化 metadata，适配层自选 schema。

## 三、RWI 待补清单（按用户明确要求排序）

1. **鼠标视觉反馈（AI 光标层）**：macOS 实现 pointer_feedback（独立叠加层绘制操作点，
   仅用于用户观感，不产生输入、不移动真实光标）。
2. **接管/占用 UX**：持久提示（如"RWI 正在接管此应用 | 停止"）+ 用户介入时让位语义 +
   操作日志可达性；明确"拒绝用户操作/介入"的边界与恢复路径。
3. **控件语义交付给 agent**（用户点名"不应该无法理解按钮/文字/工具"）：
   - scene 元素补充 **role / 可用动作列表 / enabled / value**；
   - 提供通用 `perform_action`（任意 AX 动作，而不仅是 AXPress）；
   - **弹窗/确认框作为一等对象**交付（含非前台弹出者）：所属窗口/进程、控件清单、
     可执行动作；不确定用途也要列出，由 agent 推断，而不是单工具硬碰。
4. **Electron/Chrome 后台兼容性专项验证**（文章点名的弱区）。

## 四、结论

RWI 在①③两层与公开路线一致（可交叉验证），缺口集中在"**给用户看**"（视觉反馈、接管提示）
与"**给 agent 用**"（控件语义/动作枚举/弹窗交付）两侧——这与用户本轮要求完全一致，列为下一阶段主线。
