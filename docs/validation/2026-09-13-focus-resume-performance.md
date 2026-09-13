# 焦点接续与语义重验：真实 MCP 验证

日期：2026-09-13。**本批验证通过，完整实施计划仍在继续。**

图文主入口：`outputs/acceptance-20260913/README.md`（原始截图不进 Git）。

## 现场结果

1. 升级 Windows agent 和 Mac MCP 的 JS worker 后，重新发现、绑定 Paint。
2. 用原生 MCP 的 Win+E 打开 Explorer。第一次发现遇到新窗口未显示的过渡态，
   调用方误选了已有最小化窗口；截图被拒绝，没有重复 Win+E。重新发现后选择
   实际前台的新窗口成功。这一失败保留在原始日志，不算用户手动介入测试。
3. Paint 在后台时 `mutation_ready=false`。单次显式激活约 62 ms，收到了
   精确目标的 WinEvent 前台事件，hooks 和租约均已释放；重新观察后可操作。
4. 故意提交过时前台元组，被 `foreground_changed` 拒绝，未发送原生输入，
   没有错误封锁整张桌面。当前状态重新观察正常。
5. 显式激活测试 Explorer、原生关闭；新发现完整且原窗口不存在。未关闭用户
   原有的 `agent-v1 - File Explorer` 窗口。
6. Paint 的 Open 系统对话框中用 MCP 输入已保存副本的路径并按 Enter，按原生
   owner 关系回到 Paint。重开后整个原生窗口 PNG SHA 与升级前完全一致。

## 性能口径

同一个 Pencil 工具按钮（不在画布落笔）：新版本三次 `act_checked` 分别约
118 / 787 / 796 ms；含 claim/release 的动作段约 124 / 793 / 837 ms。
对应观察仍约 745 / 1712 / 1845 ms；UIA 完整遍历仍是主要剩余开销。

旧版只获得一个成功基线动作，随后 UIA provider 超时使目标未返回。因此不是
三样本 A/B，也不能据此宣称整体提速比例或与 Codex Computer Use 打平。
同 cell 实际调用耗时不包含模型两次工具调用之间的思考/调度间隔；整轮统计
必须另读完整任务时间线，不能将本表当作整轮耗时。

## 回归及边界

- 默认 workspace：316 passed / 3 ignored；all-features：403 passed / 4 ignored。
- JavaScript 26/26；fmt、workspace all-feature clippy、Windows ARM64 clippy/build
  通过。新增公开负向回归，没有降低旧测试或权限门槛。
- 独立审查发现并关闭 SDK 迟到观察和在途 bind 的旧 frame 竞态。
- 真正人工介入、异常 HWND 重用及 UIA provider 的所有动态行为，不由离线
  或本次程序发起切换推断为已覆盖。
- 不使用 AttachThreadInput、模拟 Alt 或重新发送激活来抢焦点。

技术证据：`work/resume-and-performance-20260913/native-evidence.json`，
`agent-install.json`、`mcp-install.json`、`all-features.log`。
Windows agent SHA：`738dd0b5a1584e320c815050a4e2d9aca24d31e4c053774d006991c252ea73e0`。
实际实例：`f58bdbb8-01c9-440c-9488-370e40fb19f0`；新 SDK 可用不代表旧 MCP 父进程
已经整体热更新。配置及既有启动任务未变。
