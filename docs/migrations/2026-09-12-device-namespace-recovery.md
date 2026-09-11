# 设备 namespace 的初始化与受验证恢复

Device RPC 的执行代际不进入耐久 request 去重键。一个 state root 固定绑定一个
DeviceId；不能为了换任务、换 Host 或恢复输入而删除绑定/账本、换 ID 重放。

## 正常初始化

`device-namespace.json` 仅以完整内容发布：唯一 pending 文件 → 完整写入 →
文件 sync → no-overwrite hard link → 目录 sync（Unix）→ 删除本次 pending 链接。
未提交的 pending 不是权威绑定，重启可重新完成初始化，同时保留旧尝试证据。
已有完整绑定直接校验；已有损坏绑定不会按当前 config 静默重建。

## 已有损坏绑定

仅用于 operator 已确认元数据损坏、掌握**之前可信且已提交的绑定备份**的离线维护。
当前测试 Windows agent 的绑定健康，未对其运行此恢复命令。

1. 保留原 state root、坏文件、所有 request/result/recovery 记录。
2. 从先前可信备份清单取得 backup SHA；不得根据希望使用的 DeviceId 临时生成
   “备份”。同时记录当前损坏文件的精确 SHA，防止错误对象被替换。
3. 使用和服务相同的私有配置及 OS 文件访问身份。服务必须已经停止；工具必须
   获得同一 `service.lock` 独占锁，活服务会被拒绝。该命令不代替管理员授权。
4. 创建私有 repair spec（以下路径与 SHA 均需替换为已核验的真实值）：

```json
{
  "config": "/absolute/private/agent.json",
  "damaged_sha256": "SHA256_OF_THE_EXACT_DAMAGED_FILE",
  "committed_backup": "/absolute/private/previous-committed-namespace.json",
  "committed_backup_sha256": "PREVIOUSLY_TRUSTED_BACKUP_SHA256"
}
```

运行原 Rust agent 的 operator 入口：

```text
rcu-agent --repair-namespace ABS_PRIVATE_REPAIR_SPEC
```

Windows 使用对应的绝对 Windows 路径。此入口不是 MCP 工具，不能被模型当成
一次业务操作失败后的自动恢复流程。它校验备份 SHA、备份 DeviceId、当前坏文件
SHA 与服务独占所有权；完整的既有绑定不能被该接口重新绑定。
损坏 inode 先保留为 `namespace-damaged-*.evidence`，再原子替换绑定；账本不变。
缺少可信备份时停止于证据保全，不从截断字段或当前配置猜测原身份。

测试涵盖六个真实子进程退出边界、错设备/错坏文件 SHA、可信备份恢复、损坏证据
保留、活 owner 拒绝，以及实际离线 CLI 合成元数据恢复。Windows 同样实际运行
了相关 unit/integration executable；不将进程退出测试扩大成断电持久性证明。
