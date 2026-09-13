# 新工具加载、只读截图与有界多步操作

**已在用户重启 Codex 后，通过当前任务真正加载的 RCU MCP 验证。**
主图文入口：`outputs/acceptance-20260913/README.md`，按步骤 01–10 查看。
整体实施计划继续，不把本批当作全部功能完成。

## 实际结果

- 新工具 `computer.capture_visual` 已出现在 Host 目录。首次 include 约 **155 ms**，
  1024×555 / 77,543 bytes；图中可见真实 RCU 指针，不是后期添加。
- exclude 三次约 **136 / 86 / 80 ms**，各 71,696 bytes，三张 PNG SHA 相同；
  每次 suspension ticket 不同，释放后计数均为 0。单帧不证明 FPS 或连续动效。
- `capture_visual` 不创建 frame、不授予输入权。插入一次纯视觉读取后，SDK
  原 grounding frame 仍保持；随后原生 checked 点击 Pencil 成功、租约释放。
- 新 SDK 的三次 Pencil 工具按钮批次全部返回并释放，批次 **6.54s**；其中三次
  UIA 观察约 1.65–1.68s/次，三个动作约 0.51–0.53s/次。不是整体任务时间，
  不包括模型在工具调用之间的间隔，也不是三笔完整绘图或与 CUA 打平。
- 512×512 原生画布 tile SHA 仍为
  `ab04a812882dc0a16fa93ab02d78cc8aaf172627a3d7d5b66f3379503fa7ae75`，
  与本轮开始前一致；没有在用户原画上落笔。

## 保留的失败，而非隐去

1. 替换二进制和 JS worker 后，旧父 MCP 仍拒绝新名称：`js_unknown_capability`。
   发现/绑定正常，新截图尚未执行。用户重启后，新目录和实际截图才通过。
2. 早先 Pencil→Fill→Pencil 序列在第三步没有得到可用目标，调用方停止并释放。
   前两步有 durable 原生回执，第三个 ID 查询无记录；结合当时调用轨迹判断未
   发送第三步，不能单靠“无记录”推断任何未知输入绝未发生。没有重放前两步。
   重启前没有保存该次完整观察，缺目标的确切原因尚未确定；不能冒称已定位为
   焦点或 UIA 超时。后来根据新观察，用独立新意图恢复 Pencil。

## 回归与边界

- 默认 workspace：332 passed / 3 ignored；all-features 完整日志：419 passed / 4 ignored。
- JavaScript：41/41。独立审查修复了迟到分页/预览、超时和并发错误丢失原回执的
  问题；最新 SDK 也用真实 QuickJS worker 的离线 mock 验证，不冒充 Windows 输入。
- fmt、workspace all-feature clippy、Windows ARM64 clippy/build、架构 ratchet 通过；
  ratchet 通过不代表已清除所有架构债务。另一个 verifier 独立运行 20 项公开回归。
- metadata 32 KiB 已覆盖完整 VisualObservation；源图、PNG、base64 各有独立上限。
- 原生 presenter 的首启并发/极端清理失败仍只有模型测试与源码审查，不声称全部
  Windows 故障场景已触发。观察到焦点变化会停止，但未观察到的往返介入仍有限制。

Windows agent SHA：`3daf39ed271f433dd974d6a618dca5505a732e2f649c27cbd639fa20452935cc`。
Mac MCP SHA：`76936d7591c18fbe89478ec0286cf13799f0bfa89928412a7f6666f21acd39ff`。
既有配置、启动任务未变，回滚件均保留。长日志在
`work/resume-and-performance-20260913/`，不提交截图、二进制或运行态数据。

## 当前 Host 的补充只读采样

本补充使用后续 `45ae1911…` Host MCP 与 `a0472225…` Windows agent，
不是上节历史版本；部署完整SHA见同日 `2026-09-13-privilege-roi-stop.md`。

后续同日重新发现时，Windows 桌面已为2582×1824，Paint窗口已变为2582×1656；
本工具没有修改显示模式。按新原生记录重绑，再读取三帧，没有发输入或沿用旧坐标。
原生 MCP `observe` 三次约712/591/616ms，其中 UIA 遍历约491/428/433ms；
另一次1024px `capture_visual` 约140ms、178549字节，不走UIA、不生成输入frame。
全部在同一JS调用内完成，整体约2.315s，包括发现和绑定，但不包含模型返回后的间隔。

这是当前应用状态的一组小样本，不是跨版本A/B：窗口尺寸、应用重启与控件状态
都与早前不同。语义覆盖仍为depth 8不完整；不能从39个已加载控件推断全树只有39项。
截图见主图文第36步；原始计时为 `work/resume-and-performance-20260913/`
下 `observe-baseline-current-host.json`、`observe-baseline-current-uia.json`。
