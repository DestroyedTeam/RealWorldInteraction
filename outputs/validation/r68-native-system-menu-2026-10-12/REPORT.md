# R68：真正NSOpenPanel菜单的语义操作与可视缺口

1. 从已成功151命令模板启动独立152（PID74718／根44766／虚拟4）。原生oracle第一采样已active/key/frontmost，**不是后台no-raise验收**。不重做已完成CmdTab/Live恢复案例。
2. Files AXPress一次uncertain/-25204；保持同原进程只读等待，真实sheet44776/open1/own-parent/display4随后出现，没有重放Open。
3. 对sheet内Grid view菜单按钮AXPress一次returned；fresh snapshot新增AXMenu与Show Sidebar/Icons/List/Columns等AXMenuItem，exact-PID catalog仍4窗，无菜单独立CG窗证明。选List一次后独立AX回读Grid view→List、icon view/AXList→list view/AXOutline，不能只靠returned。
4. **两个缺口**：SCK原窗口图未出现菜单像素；after-menu与after-List两个原PNG SHA相同，像素/控制非原子且独立popup未捕获，不称List视觉变化已证实。随后一次Icons恢复请求returned，但即时回读仍List；在另一次Cancel前没有等到恢复确认，不宣称恢复成功，也不重放Icons。
5. 实际Host MCP Apps tab4已显示，Live只合成已验证root+sheet两个provider相同层，没有把菜单的AX控件存在当视觉层证明。Stop真实stopped/清图/隐藏，但最后worker_done=false；随后binding detach，不能以这个旧status称worker结束已独立验证。下一轮关闭观察必须先等workerdone再detach。
6. 真正sheet Cancel另一次returned，真实response0/visiblefalse/解除父关系；源仍可snapshot，然后独立Close exit0/expiry0，两binding detach。7个原durable请求全部terminal/released，无pending/cleanup uncertainty/replay。未打开用户文件。

## 原图（按真实顺序）

![菜单前](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r68-native-system-menu-2026-10-12/01-before-native-menu.png)

![AX菜单存在但窗口图没有菜单层](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r68-native-system-menu-2026-10-12/02-after-menu-AX-controls-available.png)

![List语义已变化，像素仍与前图相同](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r68-native-system-menu-2026-10-12/03-after-List-action.png)

![Icons恢复请求之后，语义未确认恢复](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r68-native-system-menu-2026-10-12/04-after-Icons-restore-request-unconfirmed.png)

## 下一项修正

旧Agent的native_semantic_actions未声明控件已实际公开的AXShowMenu/AXCancel/AXPick。原生dispatcher本已支持精确action字符串及exact object/window/provider/state重验；扩展明确能力，不偷偷换成鼠标、激活或重放。AXPick在本机SDK仍列Obsolete，所以仅当真实provider显式advertise且caller明确选择才可用，不默认用它替代AXPress，也不承诺必然修复Icons。

依据：[Apple AXPick](https://developer.apple.com/documentation/applicationservices/kaxpickaction)、[AXCancel](https://developer.apple.com/documentation/applicationservices/kaxcancelaction)、[AXShowMenu](https://developer.apple.com/documentation/applicationservices/kaxshowmenuaction)。直接页面仅JS壳，网页给出的Markdown链接实际404已保留；本机Apple SDK原声明已保存为R69/SDK_AXActionConstants.h，不伪造完整网页正文。

本轮旧Rust逐字节版本的R66工程门禁保留；新R69能力声明/测试必须运行自己的完整门禁，不能挪用774通过。全F01–F16仍未完成，尤其foreign-provider菜单视觉、冷態tracking、隐藏最小化/DnD/压力等。
