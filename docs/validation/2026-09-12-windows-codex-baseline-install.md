# Windows Codex 原生对照：安装完成，绘图尚未验收

日期：2026-09-12。目标是用户指定的 Windows 11 ARM64 虚拟机。
此文只记录安装与验收准备，不把 SSH 安装当作 Computer Use GUI 验收。

## 平台差异：已知与未知

官方文档描述 Windows Computer Use 使用活动桌面并移动实际鼠标，Mac 支持
后台操作。这是交互模型的差异，不证明某一版延迟更低，也不能据此反推私有
RPC、动画、轨迹调度或流式传输实现。
[官方 Computer Use 文档](https://learn.chatgpt.com/docs/computer-use)

正式对照保留 Mac Codex → 原生 Mac 绘图软件、Windows Codex → Windows Paint
两条路径。JS Paint 仅作为探索记录。RCU 的设备侧可见 Pointer 跟随仍未完成；
单张截图中的箭头/光晕不能证明连续运动或其延迟。

## 安装来源及验证

官方桌面文档目前使用 ChatGPT 应用名称，应用内含 Codex 模式；Windows 包身份
仍为 `OpenAI.Codex`。使用官方部署页给出的 Store 签名 ARM64 MSIX，而非第三方包。
[Windows 部署文档](https://learn.chatgpt.com/docs/enterprise/windows-deployment)

- 下载地址：`https://persistent.oaistatic.com/codex-app-prod/ChatGPT-arm64.msix`
- 大小：767603171 bytes。
- SHA256：`9e69aa06823922eb0b2cd1b5cf1ed55f026a94aa8521aee572253a3ee02e9329`。
- Windows `Get-AuthenticodeSignature`：Valid；SHA256 与 Mac 下载件完全一致。
- 包名：`OpenAI.Codex`；版本：`26.903.8094.0`；架构：`Arm64`。
- 包注册：`Status=Ok`、`SignatureKind=Store`。
- Package family：`OpenAI.Codex_2p2nqsd0c76g0`。
- 安装位置：`C:\Program Files\WindowsApps\OpenAI.Codex_26.903.8094.0_arm64__2p2nqsd0c76g0`。
- Mac 现有应用版本：`26.908.40834`，build `8881`。两端版本并不相同，后续不能
  将未经控制的差异全部归因为 OS 或 Computer Use 实现。

安装前没有 Codex 包或进程，Windows 用户目录也没有 `.codex/config.toml` 和
`.codex/auth.json`。未复制、读取或输出 Mac 的登录凭据；未改任何 Codex 配置。

## 安装经过与清理

1. `winget` 的 Store 源要求额外接受交易条款/地区设置；未接受或修改它，转用
   官方明确支持的直接 MSIX 分发方式。
2. 已有 SSH 的 Session 0 执行 `Add-AppxPackage` 失败，错误 `0x80070005`；
   包日志定位到 PLM 初始化及 PackagesInUseClosed 阶段，注册未成功。
3. 使用用户交互 Session 1 的一次性、无触发器、**Limited** 权限安装任务。
   安装脚本先验证当前会话、包哈希和签名，然后仅执行标准 `Add-AppxPackage`。
   此路径成功，不需要关闭安全检查、修改执行策略或提升为管理员。
4. 独立读取包注册确认成功。临时任务
   `RealComputerUse-Codex-Install-20260912` 已删除，复查不存在。

已有 RCU agent、其启动任务、SSH/firewall 设置没有改动。PowerShell 仅用于
一次性安装部署；它不提供 GUI 输入，不是 RCU 后端，也不计入原生 GUI 验收。

## 登录与实际验收的交接

Windows 桌面目录：
`C:\Users\vincentadamnemessis\Desktop\RCU-Codex-Benchmark`。

- `START-HERE.md`：给 Windows 本机 Codex 新任务的独立验收说明。
- `nemessix-reference.png`：从 Mac 应用图标提取的参考图，仅供观察，不能导入
  Paint 冒充绘图。SHA256：
  `2ade730d26cadf94fe2930b3cfa94789cda87b6f6faddf22206398db67a37f18`。

用户需要在 Windows 应用正常界面完成登录和 Computer Use 授权，再交给 Windows
本机 Codex 执行。Mac 当前的任务管理连接没有 Windows Host，不能把 Mac 新建
任务或 Mac → RCU 操作冒充 Windows 本机 Codex 的 Computer Use。

验收说明要求先执行短连续笔画/撤销，再用 Paint 工具绘制图标；分别验证动态
Pointer、绘图、GUI 保存、文件重开。保留原有 Paint 未保存草稿，不覆盖或丢弃。
总耗时包含模型/调度/编排间隔；没有证据的动态跟随、帧率、纯模型思考时间和
连接内部细节都标为未测。不得用 Python/PowerShell 输入或网页画布替代。

**当前状态：安装已通过；账号登录、插件可用性、Windows 原生绘图、动态 Pointer、
保存重开及两端性能对照均未通过验收。** 未启动或读取 Codex 自身 UI。

## 本机证据

工作目录 `work/windows-codex-baseline-20260912/` 保留包元数据、签名结果、初次
失败日志、Session 1 安装脚本及 `install-result.json`。安装包、参考图、临时
脚本和原始运行状态不提交 Git。
