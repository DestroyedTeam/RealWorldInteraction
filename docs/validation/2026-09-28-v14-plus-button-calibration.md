# Paint V14 搜索加号按钮校准（2026-09-28）

承接[171对象斜顶图标阶段](2026-09-28-v14-bag-stroke-calibration.md)。本轮仅操作已分配 BetterDisplay 虚拟屏上的 Paint S 文档；GameHub/游戏留在用户主屏，没有对游戏派发输入。RWI 重新 `targets → discover(PID 14404) → bind(cgwindow:2189)`，确认虚拟屏 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`、窗口 `[1536,24,1434,660]`、四项权限有效。基线源 SHA‑256 `ad33cd5357ffec92ad18bbc8358eb57391e22814739a7c9cffd21146d0b45339`。

## 顺序、实际截图与结果

1. [原参考420×680](../../outputs/validation/2026-09-28-v14-plus-button-calibration/00-reference-420x680.png)对照[开工171对象截图](../../outputs/validation/2026-09-28-v14-plus-button-calibration/01-before-171-objects.png)：参考小按钮约 x318–350/y62–92，圆角浅灰底、约1px灰十字；原稿按钮约 x312–359/y58–101，黑色十字过粗，顶部和底部残留旧灰/酒红底图。只读解包表明旧按钮仍有独立 RoundRectangle/两条 Line，而顶部条与酒红残影还涉及底层整幅 Image；没有把截图回执当作画作完成。
2. RWI 从 Paint 的颜色对话框输入 `#F2F2F2`，对话框首次落在内建屏，立即精确绑定 `Input Color` 并 `place_window` 到上述虚拟屏，稳定读回 `placement_confirmed=true`。在画布 x310–361/y50–104 画独立无边框填充 Rectangle 遮住旧大按钮；再将圆角矩形填色输入 `#E6E6E6`，设 Corner X/Y=6，画 x318–350/y62–92。打开 `Input Color Manually` 的 AXPress 返回 `-25204/uncertain`，各次只查询原操作、独立发现实际对话框，**没有重放该请求**。
3. 灰十字用色板 `rgb 0.5 0.5 0.5 1`、Smooth、宽1的**两条独立 Line**。首次横线因视图/画布像素中心差1px，RWI Undo 后核对消失，再重画；竖线首次落在 x334，Undo 后改为 x333，局部RGB均方误差由955.8降至95.2（x326–341/y69–85）。[完成按钮、尚未遮顶部旧灰条的原生截图](../../outputs/validation/2026-09-28-v14-plus-button-calibration/02-button-aligned-before-strip.png)。一次 `stale_reference` 为派发前拒绝，查询原请求确认 `native_input_dispatched=false`，新观察后用新意图继续，未重放。
4. RWI 再以独立 `#F2F2F2` Rectangle 遮住 x312–358/y47–49 的旧灰条；该3行区域RGB误差608.7→0。最终以画布焦点 `cmd+s` 保存，重新观察且清除选择手柄后取[最终原像素截图](../../outputs/validation/2026-09-28-v14-plus-button-calibration/03-final-saved-176-objects.png)，和保存前原生截图逐字节一致。[参考左／实际右8倍局部](../../outputs/validation/2026-09-28-v14-plus-button-calibration/reference-left-actual-right-8x.png)。图像为 RWI `computer.image` 从 `2868×1320` 原始单窗 artifact 裁取 `[1000,390,420,680]`，非放大预览冒充源像素；8倍图仅供比对。

固定 x305–362/y47–105 区域RGB均方误差 **1300.559→15.833（降98.78%）**；更宽 x301–369/y45–110 背景区域 **1069.479→108.551（降89.85%）**。残差主要是按钮附近旧底图的酒红线、右侧分隔与抗锯齿差异；**不能把局部提升说成整页1:1**。

## 落盘与剩余问题

正式工作源 `work/replica-verify-20260915/run6/paint-v14-card-cleanup-20260919.paint` 和[本阶段可编辑源](../../outputs/paint-v14-source/paint-v14-20260928-plus-button-calibrated.paint)字节相同：489278 bytes、SHA‑256 `485c6eed0a0a40cc3381a1d7516071f9d4defd59600ddddd4e7b430abc17a143`；两层 NSKeyedArchiver 只读核对 **176对象**（12 Image/30 Rectangle/99 Line/23 Ellipse/9 RoundRectangle/3 Text），新增的是2 Rectangle＋1 RoundRectangle＋2 Line，并非整幅截图贴图。[机器可读 manifest](../../outputs/paint-v14-source/paint-v14-20260928-plus-button-calibrated.manifest.json)。

主界面选中行仍有旧酒红外圈/横线，列表字叠压及字体差异也远大于按钮残差。底层第0 Image 是2000×679整幅栅格，列表行文字不在3个独立 DrawText 中；后续应按选中行、列表行做局部遮盖与可编辑重建，逐区验证，不做整图单层粘贴。右侧详情栏 y105附近酒红残线跨越本按钮范围，须作为更大区域单独处理。工具/窗口/显示器每次重新发现。Undo历史跨重启是否持久仍未知。

本轮无 Rust 产品代码改动、无部署、无 Git 提交/推送；不冒称重新跑过 cargo 门禁。`rtk git diff --check` 单独验证。
