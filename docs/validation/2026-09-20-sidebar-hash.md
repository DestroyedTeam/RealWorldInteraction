# V14 侧栏 # 图标推进

纯 RWI Plugin 完成1个无边框填色矩形遮盖层与4个独立 Line；保留原底图，不栅格化、不覆盖非目标图层。对照原参考的侧栏 # 符号，用原生笔画复刻。

第一次选工具遇到 desktop_busy，在派发前停止，没有绕过或删除锁；只读检查后重新观察，再以新请求成功选择。不能由事后未见锁推断最初占用者。

画完第一条线后 Paint S 自动回到选择模式；下一次拖动误移动遮盖层。立刻独立观察，并用 Undo 撤销 Move Rectangle，确认矩形恢复后才继续。后续每条线前重新选择 Line、检查 AX value=1；不把 returned 当绘制成功。

矩形遮盖层底色 #F2F2F2 与当前旧底图侧栏存在轻微色差，尚未统一；整体 V14 仍非1:1完成。四线与遮盖层均可单独调整。

绘制200ms路径的工具耗时约1083–1161ms；选择约461–673ms。保存快捷键482ms；后续观察fence=false。不是与CUA的受控性能对照，也不是整体提速结论。

![保存后实际窗口](../../outputs/validation/2026-09-20-sidebar-hash/saved.png)

通过RWI cmd+s保存原文档，终端独立核对文件变为422000 bytes，SHA256 22fe04bf8331656d15e4d7a089228791c35b7403de41c78ddf815ad19c5c1d39。保存前副本位于work/v14-stage-timings-20260920/paint-before-hash.paint，原哈希67c340468b4ca1bcdc452a1f9a29083739563ec378327553bc2e858d7aa04e3e。
