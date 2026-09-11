# M2 r2：只修本地分发，不重编 Rust

## 已证实的故障边界

r1 run `M2-task-loop-20260910-01`：2026-09-10 12:00:18 UTC ZIP stdin SHA ACK 成功；完整 `guest-setup.ps1` 塞进约 22 KB 的 EncodedCommand 后，install 和 readback 两次得到 `PrlVmGuest_RunProgram: Unable to open new session`，exit 255。12:02:32 一条短只读命令成功，确认只有 `candidate.zip`，进程/任务均 0，没有解压、Notepad、模型或原生输入。owner 已收回，handoff 已失效。

证据支持“长命令传输/参数大小相关问题”，**没有测得 Windows/Parallels 精确阈值**。不能把 exit 255 的提示泛化成 VM 整体不可用，也不能仅凭该返回码断言远端一定未执行。原 owner 另行提供的约 9 KB 长命令失败、短 argv + stdin 成功，是独立旁证，不是本任务的 Windows PS 实测。

## r2 固定设计

产品仍是 M2 **0.3.0**，Rust 源码和三份发布二进制完全不变；只有 preparation/distribution revision 变成 **r2**。r1 包、manifest、源归档及现场证据保持原样。

1. ZIP 直接包含两个 Rust EXE、完整 `guest-setup.ps1` 和 `package-manifest.json`。四个条目均是固定扁平文件；archive SHA 包含整个包，manifest 逐项声明 script/EXE SHA 和长度。
2. 第一个短入口只接收 base64 stdin，使用 `CreateNew` 写新 run 的 `candidate.zip`，完整写入并 flush 后才 ACK 实际 SHA。没有提前关闭 stdin 或自动重试。
3. 第二个短入口校验 archive SHA，仅在 `package` 不存在时解压一次，并校验脚本 SHA。install 函数**不再解压自身脚本**，任何重复/部分展开留现场，不用 `-Force` 覆盖。
4. 每个阶段的参数序列化为 ≤4,096 字节的 UTF-8 请求文件，经独立 stdin 上传并校验 SHA。input grant 另存为 ≤16,384 字节的原始 UTF-8 文件；PS 只检查长度和 SHA，不解析再序列化 grant，避免 Unicode、引号、反斜线、大整数或空白损坏。真实 grant 仍只由新 owner 确认的 fresh native tuple 生成。
5. 后续 argv 只含短 PowerShell 命令、受限路径和 SHA。短入口读取脚本 bytes、计算 SHA，然后执行**同一个已校验 bytes buffer**构建的 ScriptBlock；不先校验再重新打开另一个版本。脚本参数只有 `RequestPath` 和 `RequestSha`，用原始 production param block/validator 验证真实绑定。
6. Rust native server/pipe client、MCP、输入及独立 native oracle 没有换成 Python/PS 后台。PS 仍只做安装、有限交互会话任务启动、文件 oracle 和精确清理；实际用户目标由 Codex 选择新 Rust MCP 工具完成。

## PowerShell 5.1 执行策略

**没有切换到 `-File`，也没有暗加 `-ExecutionPolicy`、`Set-ExecutionPolicy`、注册表或全局策略修改。** r2 选择父任务明确允许的“保持旧命令语义的受 SHA 保护短入口”。源码存储在包内，但执行入口仍是原有 `-Command` 语义，不把数据目录里的 `.ps1` 作为签名/zone 受理的 File 参数。

有效执行策略及 `LanguageMode` 将在新授权中的短诊断和阶段回执中只读记录。VM 实际值当前未读取，不能说该 VM 已是 Restricted。Mac pwsh 7.6.6 的 parser/mock 不能验证 Windows 5.1、安全区域或受管 LanguageMode；若受管执行限制阻止入口，记录真实拒绝和残留、停止交回，不现场改策略或换 fallback。

相关 Windows 5.1 语义见父任务提供的 [Microsoft about_Execution_Policies](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_execution_policies?view=powershell-5.1)。此文档不是该 VM 的运行态证据。

## 长度预算：工程上限，不是平台阈值

每条真实 `prlctl` argv 都由同一 builder 生成并在派发前校验，不能用另一份短示例代替真实 wrapper。

- 完整 host argv（包括 RTK、prlctl、VM、程序和全部参数）的 UTF-8 + NUL 分隔总字节，以及完整 JSON 表示，均 ≤ **1,536**。
- Windows 风格 quoted command line ≤ **1,536 个 UTF-16 code units**。
- 任意单个 argument ≤ **1,024 UTF-8 字节**。
- 这些是主动降低暴露面的保守预算，不声称是测出来的系统限制。

正常与最大 run/字段向量合计 **74 条**。本地实际计算最大值：host argv + NUL **1,097 B**，完整 JSON **1,122 B**，Windows quoted **1,025 UTF-16 units**，单参数 **971 B**。覆盖 archive、一次展开、请求/最大 grant 上传、install/start/readback/cleanup、独立残留 audit 和所有 Rust MCP transport case。9 KB、22 KB、30 KB 旧模式均在本地派发前拒绝。

## 回执、模糊效果与残留

- 本地预算/路径/授权校验失败：未创建子进程，记录 `not_dispatched`。这与 guest exit 255 完全不同。
- 已启动 prlctl 但报 session 错误、超时、ACK 错误/缺失、响应不可解析：`guest_execution=unknown`，不能报告无副作用。
- PS 输出匹配 request SHA 的非零失败回执：保留执行阶段、最后尝试的 mutation、完成步骤和真实异常。已经写过的 ACL/fixture 不抹掉；日志中有 setup journal 也不叫整个阶段零写入。
- 每次请求先以 `CreateNew` 取得持久执行记录；失败/不确定进入 `HOLD.json`，禁止本轮继续 install/start/input。最多额外执行**一次新的独立短只读 audit**，不是重放失败命令；记录本轮目录文件/长度/SHA、是否完整、精确 candidate 进程与任务数。
- audit 也失败或时段不够：明确 `remaining_state=unknown`，不瞎填 0，不补测到期授权。native worker/输入的无重放与资源释放契约保持原样。
- 清理只匹配新 run 的 `package` 下两个 EXE 完整路径和本轮任务；不会关闭 Notepad、删除收据/state、改共享或操作其他任务。强制终止进程不冒称优雅取消。

## 本地验证与剩余工作

19 个本地测试通过；其中 Microsoft portable pwsh **7.6.6** 真 parser 对原 PS 及全部短 wrapper 的语法检查通过，**22 个实际短 wrapper 执行的参数绑定 mock**覆盖全部 phase/case、历史真实原生身份形状以及最大 16 KiB grant，原始请求与 grant bytes/SHA 完全相同。SHA 篡改、payload 过大、无效/旧授权、ACK/超时、非零回执、未知残留、本地子进程清理异常和禁止重放均有负向测试。

mock 仅执行原 production param block/纯请求 validator 和本地字节读回，不执行 Windows API。没有新的 guest/GUI/input/模型调用。Rust 31 个 build-source 文件与旧包 SHA 均重新对比一致，未重复编译、未把旧 73/1 的 Rust 回归说成本轮重新执行。

完整 Windows 分发、5.1 runtime、Notepad 编辑/保存/重开/恢复、真实输入/停止仍 `not_run`。ready 后重新协调**一整段 60 分钟**，现场仍只做固定候选验收；不使用已结束的 12:00–13:00 UTC 授权，不申请小窗口补测。
