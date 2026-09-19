# 本机 RWI Agent 运行态更名

2026-09-19，用户授权切换原本仍在运行的旧名Agent。

- 原OMP broker托管的 `rcu-agent`（PID24303）用SIGINT正常停止，确认端口释放后备份状态。
- 当前进程：`RWI Agent.app/Contents/MacOS/rwi-agent`，PID97773；launchd用户级服务 `com.destroyedteam.real-world-interaction.rwi-agent`。
- 程序位于 `~/Library/Application Support/RealWorldInteraction/agent-mac-v1/RWI Agent.app`，显示名 RWI Agent，固定 bundle ID `com.destroyedteam.real-world-interaction.agent`，本机 ad-hoc 签名。
- 配置 `rwi-agent.json`，日志 `rwi-agent.log` / `rwi-agent.stderr.log`。用户LaunchAgent为 `~/Library/LaunchAgents/com.destroyedteam.real-world-interaction.rwi-agent.plist`，登录加载、KeepAlive=false，不自动循环重启。
- 原旧名二进制/配置/PID/日志归档到安装目录的 `rollback/pre-rwi-agent-20260919/`。没有删除历史记录或配对。
- 配置字节、CA/证书/私钥SHA256及状态目录inode与切换前一致；切换前已有状态文件均存在。私有备份在本机 `work/rwi-agent-cutover-20260919/`，不提交Git。
- 新进程独占监听127.0.0.1:45129；新rwi-mcp使用原profile完成TLS握手和computer.targets调用，mac-host=available。

## 尚需用户授权的原生权限

独立RWI Agent进程没有继承OMP进程的macOS权限。当前返回 accessibility=false、listen_event=false、post_event=false、screen_capture=false；没有请求或修改TCC数据库。服务在线不代表GUI可操作。

在macOS「系统设置 → 隐私与安全性」中，为上述 **RWI Agent.app** 启用辅助功能、屏幕录制（或屏幕与系统音频录制）、输入监控。若系统要求退出并重新打开程序，授权后由本任务重启该用户服务并重新验证权限。

维护时先向当前精确PID发送SIGINT并确认停止，再用launchctl重新加载服务。服务更名不重置任何防重放记录。Paint v14尚未执行，等待实际权限和Host插件工具可用后继续。
