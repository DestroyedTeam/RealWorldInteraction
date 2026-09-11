# 性能实测：当前 Rust 候选，不做语言优越性推断

## 样本与身份

- 系统：`Darwin 25.6.0 arm64`；`rustc 1.95.0 (59807616e 2026-04-14)`，release 构建。
- MCP 二进制 SHA-256：`450ace11a345a053755fd613b1c6823c62ca6f1e1a0e80fde352025a79437fa3`；磁盘体积 2,444,272 bytes。
- 源码索引摘要：`3543c54bd177ba3b41427f5abce17f9697a1f51602d8b8ac5b0d5f69960012f2`。完整源码/二进制清单见 `outputs/build-provenance.json`。
- 原始结果：`outputs/benchmark-10000.json`，SHA-256 `00015922e74952aaeeae40963745cf351c7fd424403959ebb6bbf36c84f042ca`。
- 真实 MCP 进程启动/stdio 测量 10 次；**零 native targets**。是新进程，不是清空 OS 缓存后的冷启动。
- 合成后端 10,000 次完整循环：observe → resolve → claim → text act → observe → 精确元素独立 verify → release。每次 action 真实落盘并 fsync；verify 使用 1 个独立模型样本，不人为 sleep。

## 结果

| 项目 | 实测 |
|---|---:|
| 新进程至 initialize，P50 / P95 | 4.742 / 5.826 ms |
| tools/list，P50 / P95 | 0.200 / 0.269 ms |
| 已初始化零目标 MCP RSS | 10.44 MiB |
| 零目标 MCP 线程 / 文件描述符 | 6 / 3 |
| 空闲观测 | 2.002 s 内未测得 CPU 时间增量；不是长期零 CPU 保证 |
| 10k 完整合成循环墙钟时间 | 106.254 s |
| 10k CPU user+system / 单核等效占用 | 5.657 s / 5.32% |
| 合成 observe，P50 / P95 | 0.0103 / 0.0240 ms |
| 合成 act（含 durable fsync），P50 / P95 | 8.913 / 14.964 ms |
| 合成独立 verify，P50 / P95 | 0.0553 / 0.1258 ms |
| 每 1,000 次采样的循环 RSS 范围 | 2.31–2.77 MiB |
| 循环中线程 / FD / 活跃 lease | 2 / 3 / 0（每周期完成后的检查点） |
| metadata frames | 稳定封顶 64；本次 fixture 不含图像字节 |
| Engine drop 后线程 / FD / RSS | 1 / 3 / 2.94 MiB |
| 回执持久化规模 | 10,000 个文件，7,327,780 bytes |

## 结论边界

1. 这次固定负载未观察到逐周期增长的线程/FD/lease，metadata cache 按预算封顶；**不是证明所有原生句柄、所有时长均无泄漏**。RSS 是检查点采样，不是全程峰值。
2. FD 使用 `PROC_PIDLISTFDS` 实际填充返回的记录数，不能把 null-buffer 查询的预留容量当实际 FD；最终结果已使用正确口径。日志目录统计也改为流式，以免测到测试程序自身保留 10k 路径对象的内存。
3. 本轮不允许采集用户桌面或抢占输入，故 **真实 native capture/AX verify 延迟仍为 null**。合成 observe 不等于截图速度；离屏 2×2 CGImage/alpha 回归也不等于全屏性能。
4. 尚未进行同机、同目标、同原生负载的 C#/Python/Rust 对比，因此没有“快几倍”或“Rust 自动低内存”的结论。
5. 回执不自动裁剪，磁盘随唯一操作数增长；生产 quota/compaction/tombstone 仍待实现。本次基准只在独立临时 fixture state 下运行，结束后由测试 harness 删除该临时目录，不能据此清除真实操作回执。
6. 当前 macOS native 仍有前台固定窗口、单显示器、模态与 AX 分类的限制；Windows/Linux/远端/VM 尚未实现，不能套用本表。

## 重跑

从正式工程根目录执行，仅启动零目标 MCP 和合成引擎：

```sh
rtk cargo build --release -p rcu-mcp --locked
rtk cargo build --release -p rcu-core --example lifecycle --features fixture --locked
rtk proxy mkdir -p work/tmp
rtk proxy env TMPDIR="$PWD/work/tmp" ./target/release/examples/lifecycle "$PWD/target/release/rcu-mcp" > work/benchmark-rerun.json
```

原生性能测试另行安排授权窗口，不能把上面的合成命令改名为 native acceptance。
