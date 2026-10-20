# ADR 0004：共享指针呈现叶子包边界

日期：2026-10-11。状态：实现边界补记；沿用已批准 ADR 0002 的平台/传输分层，**不是整体架构或全部原生能力验收**。

## 已确认事实与决定

- 当前 workspace 已有 `rwi-pointer`；它输出材料像素、DPI/锚点与运动表达，不读取物理光标、不派发输入。macOS/Windows 原生 presenter 使用它；原生 FFI、窗口生命周期与输入仍由各平台包承担。
- 当前真实 `cargo metadata --no-deps --locked --offline` 与 `cargo tree -p rwi-pointer --edges normal,build --locked --offline` 表明：生产依赖只有 serde，rwi-imaging 仅为 dev-dependency。材料文件的 PNG 写出属于 cfg(test) 且默认 ignored 的离屏 study，不是生产截图桥。
- 旧 architecture/baseline.json 未登记这个已经存在的叶子包，因而报两个反向依赖及一个 membership 错误。不是删除违规结果：先保留旧规则实际 exit1，再登记精确成员与仅 macOS/Windows→pointer 两条呈现边；pointer 的本地生产依赖允许集为空。
- 新增 `presentation_only_packages.rwi-pointer.external_dependencies=["serde"]` 的**收紧门禁**：检查所有普通/build/target 生产边，包括 optional、重命名及 workspace alias，禁止新增外部平台/传输包及显式或隐式本包 build script。dev-dependencies 保留为测试边；不能因离屏 fixture 引入生产 native bridge。

## 保持的约束及证据范围

1. 新文件600行上限、每项旧 hotspot ceiling、其他九个包原有允许边均不改变；不增加宽泛 all/native 通配例外。
2. 指针 UI 表达不属于输入授权、lease、设备身份、目标选择或派发控制；消费者不会因为获得一幅 marker raster 就获得 input authority。
3. 这个 checker 是 manifest/大小 ratchet，不是语义安全分析器、供应链认证或全部源代码纯度证明。源代码符号/调用和实际依赖树是另外的评审证据；不得把 ratchet0说成 architecture_complete=true。
4. 真实 transport 默认图中 rwi-native-link 的 Windows 依赖为 optional；现代设备 RPC 的默认路径与旧平台适配 feature 分开验收。是否没有实际 FFI 链接仍须针对所构建 feature/target 测量，不从 optional 标记一概推断。
5. F01–F16 的未实现候选、framework 覆盖、composite Live family、Host 任务可见性和独立键盘介入仍按原计划记录；本次没有输入、截图、部署、Git可见性操作。

## 复核路径

- [批准基线](0002-headless-control-infrastructure-review.md)
- [共享包声明](../../crates/rwi-pointer/Cargo.toml)
- [材料与运动](../../crates/rwi-pointer/src/lib.rs)
- [工程门禁](../../tools/check_architecture.py)
- [门禁负例](../../tools/test_check_architecture.py)
- 本轮实际命令、旧结果与 rollback：项目内 work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/pointer-boundary-r29/；临时证据不作为生产依赖，也不纳入公开提交。

结论仅是：把现有纯呈现叶子的边界写准确并增加拒绝退化的测试；不改变任何输入/隐私/权限边界。
