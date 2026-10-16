# F127：保留真实失败，补齐安全错误诊断

- 唯一产品根未换：`/Volumes/ChatGPT/Real World Interaction`。
- 改动：`live_ui.html` 的 `readImage` 暴露有界 `data-error-stage/data-error-code`，区分缓存 RPC、缺元数据、PNG 校验、Blob 和解码；不显示后台消息、能力、像素或异常原文；隐藏/成功帧清旧诊断。
- 同一渲染器契约命令：BASELINE 错误阶段=false，MODIFIED=true，独立副本 ROLLBACK=false，重新应用=true，退出均0。缓存读取功能始终存在，外部网络始终false。
- 同一 Node 命令通过数：33/36/33/36；状态文档匹配命令退出1/0/1/0。全部484即时原源码哈希已在副本恢复；永久386项全局回滚与严格补丁重建通过，Root/M 回滚均拒绝。
- 本轮完整 Rust：1375通过、11忽略、48套；fmt、workspace、全目标全特性Clippy-Dwarnings、架构检查全部退出0。
- [F125 实机负例](../f125-fresh-host-live-renderer-acceptance-2026-10-16/REPORT.md)保留：v4加载成功，但会话内没有图片；原生预览/隐藏/关闭正常，全部自有窗口已清理。这不是已修好出图问题的宣告。
- F127 新诊断尚未部署；下一项F128为中性MCP构建、候选/安装协议和原子替换，不重启Agent、不改权限/证书/密钥、不要求用户配合。

完整原始命令/字面输出/状态/哈希：`/Volumes/ChatGPT/Real World Interaction/work/f127-live-image-error-boundary-2026-10-16/LATEST_ROLE_FRAME.json` 及同目录 `*.command_execution.json`。

本轮继续沿用：`/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/{MODIFIED_FILE,DIFF_FILE.patch,VERIFICATION.txt,ROLLBACK.sh}`。未提交、未推送本轮变化；全目标仍在进行。
