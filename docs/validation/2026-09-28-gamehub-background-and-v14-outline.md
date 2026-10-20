# GameHub 前台期间的 RWI 后台输入与 Paint S V14（2026-09-28）

## 边界与目标

- 本阶段只读查询 GameHub/Wine 的进程、前台和 CGWindow 元数据；**当时未截图、启动、移动或向 GameHub/游戏发送键鼠**。用户随后明确游戏保留内建主屏供本人使用；下一阶段的游戏只读截图与 Live 状态另见[独立验收](2026-09-28-gamehub-readonly-live.md)，主屏游戏窗口始终不是 RWI 画稿工作区。
- Paint S 主文档 `cgwindow:2189` / PID 14404 位于 BetterDisplay 虚拟屏 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`，属于既有明确分配的 V14 工作区。RWI 的 `targets → discover → snapshot` 确认四项权限、`mutation_ready=true`、2868×1320 原像素与 2048×942 预览。操作时 `lsappinfo` 独立确认前台仍是 Wine 游戏 PID 51946；RWI 没有抢占主屏焦点。

## 按时间顺序的真实结果

| 步骤 | RWI / 本地证据 | 结果与问题 |
|---|---|---|
| 1. 精确 PID 发现 | GameHub PID 46002、Wine Steam PID 50278 无 on-screen CGWindow；游戏进程 PID 51946 有 `CreatureCurios` CGWindow 11351，范围 `[0,0,1512,982]`，位于内建屏。 | 启动器 PID 不是游戏渲染窗口的当然归属；不能从标题或 GameHub 父进程推断。只读元数据不构成 visual-only/Live 实机兼容验收。 |
| 2. 第一个图标斜线 | 请求 `v14-sidebar-bag-outer-left-20260928-1` 返回 `dispatch=returned`，但紧接着的同尺寸原生截图相对[操作前](../../outputs/validation/2026-09-28-v14-sidebar-icon/01-before-native-sidebar-420x680.png)全幅无像素改变，Line 工具仍为选中状态。 | `CGEventPostToPid` 无 OS 效果回执；这次未证明线条产生。没有用同一个请求/几何直接重放。 |
| 3. 独立验证后台输入 | 合成点击 Paint 的 Ellipse 工具后，AX 状态从 Line=1/Ellipse=0 变成 Line=0/Ellipse=1；再用显式 AXPress 还原 Line。 | 后台工具点击确实奏效，不能将第 2 步概括成“后台输入完全不可用”；像素画布拖拽与工具点击必须分别验证。 |
| 4. 首条底线与撤销 | 新的不同线段确实画出，[试笔截图](../../outputs/validation/2026-09-28-v14-sidebar-icon/02-high-line-trial-before-undo-420x680.png)显示编辑控制柄；`Edit → End Editing` 一次成功，实像素落在 y318，比参考底边高。经 RWI `Undo` 后局部截图确认清空。 | 控制柄不是作品像素；保存前先独立确认实际位置。 |
| 5. 重画底线与外轮廓 | RWI 重选 Line，在原像素坐标重画；[校正底线](../../outputs/validation/2026-09-28-v14-sidebar-icon/03-rebased-bottom-line-420x680.png)位于 y320。后续 7 条不同线段构成斜顶外框、内侧 V 线与短横线；每条拖拽后用 AX 状态确认已进入 Select and Move，再重选 Line 清除控制柄并取下一帧。 | `Edit → End Editing` 在游戏前台时有时菜单短暂消失，某次返回 `ax_geometry_unknown` 且 `native_dispatch_entered=false`；重新选 Line 清除覆盖层更稳定，没有借助全局焦点。 |
| 6. 保存与归档 | RWI 对精确 Paint AXScrollArea 派发 `cmd+s`；磁盘文件随后变为 486795 bytes、SHA‑256 `254161a76dd7fc8759186f9cbe78bd18bc23f78b9747b460e7d745ef492356a4`。独立只读解析 Paint S 的内嵌 NSKeyedArchiver 得 **163 个可编辑对象**：12 Image、28 Rectangle、89 Line、23 Ellipse、8 RoundRectangle、3 Text。[阶段源](../../outputs/paint-v14-source/paint-v14-20260928-sidebar-outline.paint)与工作源哈希完全相同。 | 本轮净增 8 条独立 Line，保留可编辑源与已有头像/图形层。Undo 历史跨重启持久性仍未知。 |

## 视觉对比与未完成项

- [最终原生截图](../../outputs/validation/2026-09-28-v14-sidebar-icon/04-final-native-sidebar-420x680.png)对[同尺寸参考](../../outputs/validation/2026-09-28-v14-sidebar-icon/reference-sidebar-420x680.png)：该小图标在 RGB 每通道 `<80` 的深色像素 bbox 分别为 `(30,299)..(49,320)` 与 `(30,299)..(49,321)`；深色面积 84/122，交并比约 **0.373**。轮廓位置已接近，但描边偏细、参考最底行少 1px、抗锯齿/内线差异明显，**不是 1:1**。
- 其余文字重叠、酒红外轮廓、头像细节、其他图标及整体页面仍需继续。没有把截图贴成整图层。
- 针对实际回执误读，源码的 macOS capability 新增机器可读 `background_input`：`native_acknowledgement=false`、`application_effect_attested=false`、`readback_required=true`；`background_operation` 不再宣称每个非前台 app 都会接受输入。该阶段离线全仓 524 passed/10 ignored、fmt/Clippy `-D warnings`/diff check 和 Skill 校验通过；**当时尚未部署**，随后已随[GameHub Live 修复](2026-09-28-gamehub-live-cross-display-fix.md)部署并经 Host 读回。
