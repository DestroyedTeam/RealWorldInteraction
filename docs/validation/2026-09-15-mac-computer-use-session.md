# macOS Computer Use 实施与验收记录（2026-09-15）

按"步骤 → 结果 → 问题"如实记录；证据以提交号/文件路径/活体回执为准，未验项不冒充完成。

## 一、PUA Loop 验收：RCU 操作 Paint S / Freeform

- 步骤：编写设备线验收脚本 `work/pua-loop/verify-mac-apps.sh`（离线门禁 + 实机验收），
  并冻结为 Oracle（`.claude/pua-loop-state.md`）。
- 结果：**ORACLE_OK / exit 0**（迭代 8 通过）。覆盖：Paint S 后台启动/落授权屏/绑定/观察/
  点击 Add Image → 面板出现并可观察 → Cancel 关闭；Freeform 板绑定/观察 → Insert Shape →
  弹层打开 → 画布点击关闭。证据：`work/pua-loop/evidence-*.json`、`.claude/pua-loop-history.jsonl`。
- 问题（发现即修，均带活体回归）：
  1. 一次 uncertain 派发导致输入域 fence 跨会话/重启持久；根因=combined-session 陈旧键
     (118) 让 `recover_input` 永远被拒 —— 修：硬件源/按钮/修饰键仍 fail-closed，仅 stale
     session key 放行（`a821c7f` 前的恢复规则 + `3cc47e7` 附近提交）；
  2. sheet/panel 合成点击"能派发、不生效" —— 修：非主窗口点击走原生 AXPress（`2e05a56`）；
  3. 模态窗口因自身 AXModal 被拒 —— 修：聚焦窗口=交互面（`a821c7f`）。

## 二、微信窗口 1:1 复刻（Paint S × RCU MCP）

- 步骤：清画布（TextEdit 取白底 → Paint S 新文档）→ 标定画布坐标（canvas→window px +584/+258）
  → 分批 drag 落笔（外框/图标栏/列表分隔+4行/搜索框/顶栏/大图框/输入栏/3 个头像圆/2 条文字线）
  → 每批 `computer.image` 裁剪自检。
- 结果：结构级线稿成品 `work/mac-draw-replica-20260915/wechat-replica-paints-final.png`
  （15 个独立 Sketch 图层，深红描边，透明底）。
- 问题：Freeform macOS 版无可见自由画笔（铅笔藏在 Insert Shape 弹层且 AX 不可见/不可点）；
  Paint S 工具/颜色按窗口隔离、选项行不可作工具判据；残留一根早期重线未擦。

## 三、控件语义与动作执行（对 agent 的交付）

| 交付 | 实测回执 |
|---|---|
| `role`（AXRole） | 67/67 元素带值（AXButton/AXCheckBox/AXScrollArea…） |
| `actions`（AXActionNames） | 37/57（按钮=AXPress、滚动区=AXScroll*ByPage） |
| `enabled`（AXEnabled） | 34/40 报 true |
| `subrole`（bind 响应） | 标准窗口实测 AXStandardWindow；面板稀疏暴露时 null（不猜） |
| 按动作名执行 | 未声明动作→`unsupported_action`（fail-closed）；已声明动作进入原生调用 |
| 动作成功样例 | 早前同通道 AXPress 成功关闭 Paint S "Open" 面板 |

## 四、事故与修复（授权屏放置）

- 事故：宽松标题匹配误选用户 OpenCode 窗口，`bind_window` 隐式触发的归位移动改变了其位置，
  随后因窗口大于内屏而拒绝绑定（`unauthorized_display`）；已向用户披露。
- 修复（`3cc47e7`）：放置改为显式开关 `authorized_display.place_on_bind`（默认关）+
  移动前尺寸预检（`placement_impossible`）。
- 回归：对非授权屏窗口 bind → 正确拒绝且 **bounds 前后一致（unmoved=true）**。

## 五、交付与遗留

- 本日提交（部分）：`4435160` 调研入库 · `be66ca0` role · `27768f2` actions · `bd48664` enabled ·
  `b727bb1` subrole · `2e05a56` 非主窗口原生动作 · `3cc47e7` 放置加固 · `488b425` Skill 语义 ·
  行为验证 `8f977ae` 施工清单等。
- 遗留（todo 在列）：#3a 显式移动原语（施工清单已就绪）· #3b 弹窗一等交付（含非前台/跨屏例外）·
  #4 AI 光标视觉反馈层 · #5 接管/占用 UX · #6 Electron 兼容专项 · 复刻细化（可选）。
