# R90：多窗口交错发现与有界快照注册表（2026-10-13）

## 实际修复与部署

`Catalog::replace/page/select` 从单份覆盖式目录改为context-local多快照：最多8份、总2048行、各份独立10秒TTL；cursor保持自己的catalog_id。错误候选先验证再commit，不驱逐旧有效数据；完整记录相同triple却矛盾时拒绝。完整DisplayRecord纳入opaque native_epoch，显示器变化可安全立即重发现，无需等待旧观察过期。NativeMacDesktop实时PID/出生/窗口/完整record/display/session复验不变。

Agent-only部署已执行：旧PID80314→32995，实例108dc859-6575-4cc2-bd35-ed0e71ef5a54；原稳定指定签名、CA/证书/config/profile/MCP哈希均保留，四权限仍true。部署前持有原协作native lease，当前实例durable全部terminal/released并在bootout前重查；独占不宣称HID或所有Live全面空闲，本任务Live明确stopped。原子交换、启动、签名及新服务只读检查均exit0；没有恢复/清除历史不确定记录，没有要求重启Codex。

## 按执行顺序验收

1. **旧服务真实基线**：两个专用窗口source19281/CG46481，destination19283/CG46480；A发现→B发现→attachA返回window_not_discovered，attachB成功。无点击/输入。这与R88错误一致，不是人工判断。
2. **新服务实际相同流程**：基线窗口已按原10分钟lifetime退出0/0，因此新建R90B source33525/CG46505、destination33528/CG46504；A发现→B发现→attachA/B均成功。换的是必要运行身份，不按标题替代窗口。
3. 先取global cursor，再做A/B exactPID查询，原cursor仍返回同catalog_id的8行，不隐式指向B。
4. 篡改source epoch拒绝，native_dispatch_entered=false。
5. 等超过10秒，再刷新B；旧A selection仍拒绝且无native dispatch，证明B不延长A。重新发现A后明确attach成功；该额外target立即detach。
6. 对两已绑定窗口独立原始截图。随后fresh snapshot找到各自End own fixture/AXPress，唯一cleanup request关闭并detach；实际wrapper退出0/0、原请求terminal/released/no replay。当前Agent32995、四权限true、fence=false、仅原desktop target。

### 实际截图：新服务source独立回读

![01-destination-original.png](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r90-interleaved-discovery-2026-10-13/01-destination-original.png)

![01-source-original.png](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r90-interleaved-discovery-2026-10-13/01-source-original.png)

## 回归、可逆证明与已保留错误

- Catalog16项、真实MCP进程＋认证synthetic peer1项通过；后者只证明adapter精确透传/路由，不当作原生目录修复证明。
- 完整workspace818 passed/0 failed/11 ignored、43 suites；receiving fixture89通过；fmt、alltarget/allfeature Clippy locked offline、架构ratchet和10条规则测试均exit0。
- 同调用代码/输入的冻结验证副本实际执行原Catalog与新Catalog：BASELINE退出101、stdout A=Err(window_not_discovered)/B=Ok；MODIFIED退出0、A/B=Ok；ROLLBACK原hash及新增文件缺席均恢复，再退出101；REAPPLIED退出0。副本probe仅pure cache，不当作loadedMCP GUI。本轮没有生产回滚。
- 初次架构gate报告tests608>600并退出1，候选build拒绝。拆成214行原测试和398行registry测试，保留全部16case，不放宽上限；完整gate重跑0。
- 第一test helper闭包E0597已修；MCP test初次错误的adapter退休预期已改为真实agent拒绝语义；操作员snapshot.controls读取错误发生在之前readonly/attach-detach已成功之后，无输入，无重放，随后使用实际elements字段完成。
- 已确认前批cf66a9c CI run38288139388 success；不把它当本批新提交CI。

证据目录：/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/interleaved-discovery-registry-r90

完整F01–F16继续；浏览器代表性多窗口/隐私生命周期、冷后台hover/键盘、foreign-provider层级、正式offscreen Host和分层性能仍待补齐。
