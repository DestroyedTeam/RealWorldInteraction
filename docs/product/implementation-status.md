# 完整实施计划：当前状态

更新：2026-09-14。以已批准 ADR 0002 和 requirements-and-acceptance.md 为准。
用户临时发现的问题只改变优先级；提交、推送、一个成功截图均不是总完成标准。

## 正在执行的顺序

| 顺序 | 交付项 | 当前状态 / 下一步 |
|---|---|---|
| 1 | 焦点变化、窗口切换、重新接续 | 已补单次显式激活、异步完成/用户退让/最终事件收集、独立前台事实、旧观察失效；再补最小化恢复与前台分步，真实Host约105ms恢复Explorer。窗口展开仍未获得精确控件焦点时，显式点击地址栏后才发键。整Paint进程关闭、服务仍健康、GUI启动新Paint并重开原图均已验证。真实人工介入与异常窗口重用仍单列验收 |
| 2 | 语义动作性能 | 精确 runtime ID/祖先重验已落地、公开负向回归及 Paint/Explorer/文本实测通过；旧版仅一个有效基线，不声称完整 A/B。公共 observe 仍有 UIA 遍历延迟，继续第 3 项 |
| 3 | 可视化与操作通道分离 | capture_visual 已由重启后的真实 Host 加载并实测，不产生input frame；新版面积平均1024px预览保留本次细线，原生512px切片SHA升级前后相同。ROI source_region重验已实机画出第三个闭合菱形。公共语义 observe 仍需缩减全树遍历开销 |
| 4 | 连贯多步操作 | withInput 已补同target多wrapper独占、单调调度预算、重复ID/dedup停止、在途读/输入与异常留证；真实三步工具按钮批次约6.54s，另已用一个Host原生MCP JS单元完成两笔640ms轨迹，整批约5.33s，原生512px切片确认两个闭合菱形。缩略细线已修且实测；菜单focus/pointer身份表示继续修正，不称效率完全达标 |
| 5 | 完整真实验证与简洁交付 | 重启后真实Host已加载45ae版；agent重启后的首次查询自动单次重连成功，原操作记录未变。1200ms截止后的原生停止/一次租约清理成功，显式恢复仍保留不确定历史。最小化修订独立393默认/480全feature门禁通过并实测，无需再重启Host。原Paint PID2024已退出，新PID7372由GUI重开原图；文件与512px切片SHA均不变。另按新发现的窗口尺寸重绑，3次观察约712/591/616ms，只读截图约140ms，不算跨版本A/B。截图36步，历史失败全部保留 |
| 6 | Mac 受控端 | 通用 agent 装配、无选窗身份、动态CG发现/绑定、AX/CG输入与取消/回调资源已实现，独立默认424/全feature511通过；实际无窗口身份预检通过。R1长文本/双击自身状态变化导致中断的问题已审出并修复。本机用户级开发版已部署：新私有配对（agent-mac-v1）、loopback mTLS 连通、运行进程 accessibility/screen_capture/post_event/listen_event 预检为真，证据见 work/macos-deployment-20260914/；Host 原生 GUI 验收待做；Pointer/drag/ROI/capture_visual/lifecycle等仍未实现，详见本批验证文档 |
| 7 | 跨端和架构收尾 | agent_binding从1592行纯拆分至根222行/子模块≤310行，76函数体和12类型保持；继续剩余模块债务、生产feature、多端故障隔离与资源上界。已有离线通过不能替代未测真实平台 |

## Mac 绘图应用实测（2026-09-14，进行中）

- 目标：内建屏上用 Freeform / Paint S 临摹窗口示意图并对比 MCP 体验；证据与对比表
  见 work/macos-mcp-draw-comparison-20260914/（work/ 为中间产物，不提交）。
- Freeform：后台观察/元素点击/拖拽派发可用；但"Draw with Pen"位于 Insert Shape 瞬态
  面板（AX visibility=unknown/occluded），element/pixel/checked 三路径分别以
  `visibility_unknown`/`stale_reference` 拒绝，无法进入画笔模式；选择模式下拖拽只框选。
  仿图未完成，画板仅留此前插入的少量对象（freeform-board-20260914.png）。
- Paint S（沙盒）：窗口/画布可用；形状工具可创建对象（图层与尺寸可读），但默认
  Border:None 且颜色/样式面板为独立瞬态 UI 不可达；新建文档模态尺寸面板不可完成
  （`capability_unavailable` owner-chain 未实现、sheet AX `window_missing`），只能以
  打开已有图片绕过。仿图未完成（paint-s-window-region.png）。
- 对照 Paintbrush（非沙盒）同一路径已完成 29 笔仿图与形状版仿图 → 差别主要来自
  应用 UI 结构（AX 暴露度、瞬态面板、沙盒、模态窗口）。
- 已修复并部署（本机 agent abd74948，离线门禁 fmt/clippy/test 全绿，提交 84bacf9/
  2fb57b5/1aa6d74）：
  ① 身份改用 boot 秒级（修复 kern.boottime 微秒漂移引发的全量 session_mismatch）；
  ② 窗口解析支持 AX 子级 sheet/对话框（修复模态面板 window_missing，使其可观察可操作）；
  ③ 窗口发现/绑定携带所属显示器（id/built_in/bounds）。
- 显示器归属实测：Paint S 当前唯一文档窗口 display={id:1,built_in:true}（内建屏）。
  落点机制：macOS 主显示器=外接 IG27Q，未定位的新窗口由系统/应用放在主显示器
  （首启 Start 面板曾出现在扩展屏），随后应用会恢复其保存位置（Prefs 中
  StartOpenPanel 保存的是内建屏坐标）。macOS 尚无启动/放置原语
  （launch_application 未实现），Host `open -a` 只能接受系统落点。
- 待实现（用户要求，高优先级）：受控的窗口移动/放置原语（将目标窗口强制放到授权
  显示器），并把"仅授权显示器"作为默认可操作门槛；③ 的识别已就绪，可作为策略输入。
- 本轮未完成：sheet 修复的端到端模态验证、独立验证子会话、Mac 视觉反馈（AI 光标层）、
  正式验收文档与总提交。

## 已有基础，不重复从零做

- 独立 headless Rust Windows agent、设备 mTLS、全局 MCP；当前通用 Skill 为源码/项目镜像，旧 Plugin 包仅属历史验收，不冒充全局 Plugin/Skill 安装。
- 原生 screenshot/UIA/鼠标/键盘、菱形常驻 Pointer、单笔多点拖拽。
- 同资源租约、持久化不重放、连接复用、compact 元数据与原图按引用取回。
- 原生 Paint 图标绘制；后续 PNG 副本保存/重开哈希一致；Explorer 原生操作。
- Git 主线根 workspace、功能分支、可回滚小提交；正式 UIOS 验收未宣称完成。

## 完成口径

- 每项必须有实现、回归及适用的真实入口证据，才移为完成。
- Linux/移动端/IoT 遵循 ADR 的平台分级，不提前创建空实现或承诺不可用 OS API；
  尚无已分配实机的能力不填“已验证”。它们不阻止已分配 Windows/Mac 的持续实测。
- 不请求虚构的五分钟/一小时总槽位；当前 VM 授权无理论时限。实际单次操作、
  租约和取消仍有界；用户正在输入/切焦点时退让，不持有输入租约做离线开发。
- 最终主文档以“步骤—截图—结果—问题”顺序展示；技术日志、hash 和长表放附录。
  截图不提交 Git，不将旧截图标为本轮新验证。

**整体仍在实施中。** 本文件是持续状态表，不是缩小范围后的完成声明。
