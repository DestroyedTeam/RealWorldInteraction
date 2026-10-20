# R70：当前新版MCP显式菜单动作实机验收

1. 独立153（PID90874/root44814/Virtual4），从成功152命令模板启动。Files AXPress一次uncertain/-25204，同原进程只读后来出现真实sheet44824/open1/own-parent/display4，无旧输入重放。
2. AXShowMenu：fresh Grid view明确advertise，caller显式指定；returned后实际快照有AXMenu与Icons/List/Columns。exact-PID catalog仍4窗，不证明独立菜单CG窗/foreign-PID。
3. AXPick List：真实菜单项advertise后仅调用一次，独立AX回读List/AXOutline list view，非回执即goal；原图仍相同，不称像素已变化。
4. Icons恢复：新explicit ShowMenu、fresh Icons AXPick一次；第一回读仍List/菜单，保留它，只有read-only500ms后第二次才Grid view/AXList icon view。没有重放。需要settle；不同run/action不能证明AXPick比AXPress更好，R68没等到效果也不证明旧API必坏。
5. AXCancel：另一个新ShowMenu后，fresh AXMenu自身advertise AXCancel，调用一次；同窗树不再返回原菜单/菜单项，Grid保持。但coverage partial/absence_proven=false，结论限于该AX投影，不称全局视觉缺席。
6. 实际Host v2 tab5可见、Live root+sheet2层。单独File Cancel response0/visiblefalse/解除parent；Live Stop先只读等worker_done=true、清图隐藏再清理绑定，源仍可读，然后源单独Close exit0/expiry0。两detach、9durable全terminal/released/无pending/cleanup uncertainty/replay，pointer feedback隐藏。

## 按真实时间顺序的原图

五图原始SHA全部66e6b962bf1005952307347e76ff3b2da46e77750841d3fb200578cf3eb8b3b6。不能把相同像素当菜单层/视图视觉变化成功；语义状态已分开读回。不重绘、拼合或伪造原图。

![动作前](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r70-native-menu-actions-2026-10-12/01-before-explicit-menu.png)

![AXShowMenu后原窗口图](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r70-native-menu-actions-2026-10-12/02-AXShowMenu-controls.png)

![List语义回读后的原窗口图](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r70-native-menu-actions-2026-10-12/03-AXPick-List-readback.png)

![只读等Icons恢复后的原窗口图](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r70-native-menu-actions-2026-10-12/04-AXPick-Icons-restored.png)

![AXCancel菜单后的原窗口图](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r70-native-menu-actions-2026-10-12/05-AXCancel-menu-readback.png)

## 范围

三个新增显式能力及具体语义效果接受。不是后台no-raise：fixture第一原生采样已active。没有人类输入/用户眼见宣称；捕获与AX投影、菜单/provider视觉继续追查，不放松guard或以同PID猜测。

R69同源11门0/workspace776/receiver72/UI7。同四角色173路径/原97不变，native_semantic_actions源配置BASELINE1/MODIFIED0/ROLLBACK1/REAPPLIED0及精确hash恢复；不是生产回滚。完整F01–F16未完成，冷态hover、foreign-provider/Space/hidden/minimized/DnD/压力等保留。
