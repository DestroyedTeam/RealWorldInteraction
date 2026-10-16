# macOS 原生 AX 对象与目标级重验证：第二轮候选

日期：2026-10-02。依据[两会话审计](2026-10-02-macos-computer-use-comparison.md)，Paint复刻保持暂停优先级。本轮无应用启动、激活、截屏或输入；不操作用户正在使用的游戏。

## 问题与修复边界

Chrome已找到的 `Create Custom App` 在动作前被无关AX深分支堵住。旧 `exact_action_element` 要重扫全窗、证明几何/role/identifier摘要唯一；遇任一16层分支未遍历完就拒绝。初次观察本来又是partial tree，形成“已找到但不能操作”的落差。

本轮改变**对象身份和动作重验证**，不提高16层/512节点观察预算、不把unknown visibility当作无遮挡、不新增自动focus/点击重试，不宣称能发现所有更深层控件。初始完整观察及动态capture settling仍保留。

## 实现

1. `native/objects.rs`：每个已选原生backend拥有最多1024条对象记录。native对象相等性是CFEqual，不是公开属性摘要；不同对象即使所有属性相同也取得不同不重复serial。根window对象替换会清空记录，serial不重置。属性/几何变化、淘汰后重现也不能恢复旧id；耗尽明确拒绝。没有全局跨窗口/上下文cache。
2. `native/objects/native.rs`：只保留经 `AXUIElementGetTypeID` 检查的AX代理。普通 `Cf`仍非Send/Sync；窄类型 `Ax`只实现Send、不实现Sync，由backend独占执行和registry mutex约束；native调用方得到的拷贝仍是普通非Send句柄。没有把任意CF值、AppKit view、observer或run-loop对象声明成线程安全。
3. 动作定位直接取观察时保留的对象。新鲜 `AXWindow` CFEqual关系可以证明标准window包含关系；sheet/popover则要求实际AXParent链到达选定根。循环、缺失、32节点/500ms限制、取消、进程替换全部拒绝；不搜索其他同属性控件。跨PID仍只允许原有已签名系统文件面板delegate，不扩大到任意helper。
4. `NativeMac::revalidate` 对语义ref只读取该对象及原窗口/session/process/layout/modal事实，保留原role/value/enabled/actions/几何/可见性比较。与全树观察共用 `observed_element`，不为native action改写visibility。Pixel ref仍走原完整图像重验证。
5. 每次retained probe恢复0.05秒AX读取预算，避免前次 `AXPerformAction` 配置的0.5秒预算残留；动作预算和未知效果分类不变。原lease、durable请求指纹、no-replay及围栏未修改。
6. 原生binding携带已经读取的window title、`document_state:null`，不创造业务oracle；旧native epoch本来绑定同一title，不增加AX调用。typed snapshot和JS summary显示已观察的window/PID/title，缺失title不猜测，visual-only不创造binding。这只是改善识别当前窗，**尚非独立多窗工作集**。

身份依据：Apple说明AXUIElement可使用CFRetain/CFRelease/CFEqual；AXWindow关系指向包含它的window，sheet并非window；AXParent是原生层级关系。[AXUIElement官方文档](https://developer.apple.com/documentation/applicationservices/axuielement_h)、[AXWindow](https://developer.apple.com/documentation/applicationservices/kaxwindowattribute)、[AXParent](https://developer.apple.com/documentation/applicationservices/kaxparentattribute)。CF并非所有对象/修改都自动线程安全，因此候选保持类型检查、独占执行与非Sync约束。[Apple线程安全说明](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html)。这些是代理/属性证据，不是原生控件连续生命周期或OS原子性担保。

## 离线验收

- Registry负例：同属性不同对象、对象替换、窗口替换、几何变化后回到旧几何、淘汰、serial耗尽。
- Ancestry：只读取目标祖先，不提供兄弟/children枚举；循环、未知parent、深度超限、取消均拒绝。
- 跨层fixture：Registry → exact target revalidate →真实 `Engine::act_checked` → durable receipt。两个同名同属性按钮id不同；无关动态兄弟变动后只有目标被读，派发一次；重复请求及重建Engine后返回原回执，不再读对象或输入。目标替换、换根、脱离根、disabled、action撤回五种负例全部零输入。
- 专项对象测试 **8 passed**；纯像素专项 **6 passed**。初次测试有两处fixture构造遗漏新field、一个测试误把属性变更后id当成不变，均已修正；没有把失败轮次算作通过。
- 新增snapshot窗口投影Rust/JS测试，未知/visual-only保持null。
- 最终全仓 **553 passed / 10 ignored（41 suites）**、JS **47 passed**；fmt、all-targets/all-features locked offline Clippy `-D warnings`、diff检查、Agent/MCP release build通过。候选以同一本地leaf严格签名验证；已安装Agent SHA仍为 `2045fc748f334afd3cf3ad6bb2e04491d85301e39d2ec9b0765fef58108d7ab4`，未替换。源码/候选二进制摘要及门禁见[候选manifest](../../outputs/validation/2026-10-02-retained-ax-target/candidate-manifest.json)。离线测试不替代Chrome/飞书/游戏真实GUI验收。

## 游戏补充样本：局部像素诊断，不是放宽准入

来自游戏会话本轮反馈，尚未由本任务独立读取原帧/回执：owner PID70878/CG5398、1800×1169 points，preview2048×1330，empty-AXWindow。锻造入口确已打开；关闭X附近一次在core前检查拒绝，另一次有AppKit准备event1/请求input0、最终guard `grounding_matches=false`、local anchor存在。该会话已停止重放并请用户物理关闭；本任务未给游戏派发任何动作，也未重启其Agent。

“X看起来静态”不证明整个96px patch稳定。新候选对**同一个比较器**输出中心最大delta、变化cell数、总delta及失败维度：dimensions、desktop point、transform、decode、中心变化、周围变化超预算。报告只有聚合指标，不含pixels/文字、不产生frame、不授予输入authority；只在失败路径附加诊断，不增加native capture。现有3/32/384/16阈值完全保持。

fixture专门覆盖“中心按钮不变、周围战斗区域变化超限”，仍拒绝；中心目标变动也拒绝。下一次部署后的合法新样本才可判断真实拒绝原因，不能把这份fixture当作游戏已修好，也不能重算旧回执或凭静态X放宽规则。

## 部署与剩余工作

- stable本地leaf为 `24B346A95EC8F450F7D8433A2E1876568DBCB2F8`；候选APP在work目录打包，严格签名验证，DR仍是原identifier+leaf。没有上传私钥/修改TCC/配对/已安装服务。
- 当前Agent仍是旧版本；生产Agent重启会使旧bindings失效，游戏会话仍使用它，因此已询问部署时机及独立Chrome虚拟屏测试窗口。Agent修复与MCP/Skill输出更新可分批部署；后者需重新加载相关MCP/插件，不能把替换Agent说成已更新旧MCP进程。
- 尚待：Chrome实际已发现控件、系统sheet、后台输入及delegate同等级负例；CF身份稳定性/内存与真实单步时间；多窗独立handle与related window集合；动态capture/AX coverage、窗口级human activity定位。没有默认扩大UI授权，也没有提交/推送。
