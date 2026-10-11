# Paint S V14：第 4–6 行独立头像（2026-09-25）

## 目标、来源与隔离

在 BetterDisplay 虚拟屏（UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`）上的正式 Paint S 文档 `paint-v14-card-cleanup-20260919` 中，为第 4–6 行补入三个**可独立编辑的 Image 图层**。原始非公开头像只取自已有参考图 `work/replica-verify-20260915/attachments/attach-3-bkkRer32.png`（2000×1193），按参考中头像列 x≈205.5、行距124px，分别精确裁切 x=162、y=607/731/855 的 82×82 区域，并用 `sips` 确定性缩放至 48×48。原裁图、导入图均保留在 `work/replica-verify-20260915/assets/`；这不是用生成模型重绘头像，也不是把整张微信截图烤入底图。

每行 GUI 操作都由**本任务加载的 RWI MCP**完成；终端仅准备头像素材、备份 `.paint`、只读解析保存归档及复制阶段快照。操作前从已保存源复制 `work/paint-v14-before-row{4,5,6}-20260925.paint`，不以 Undo 栈代替落盘备份。mac-host 四项 TCC 均有效，目标绑定为 `cgwindow:2189` / PID14404 / 当次 BetterDisplay 窗口 bounds `[1536,24,1434,660]`。

## 按时间顺序的真实结果

1. **第 4 行。**曾尝试只在参考窗露出的状态栏发送 RWI 点击将其置前；第一次在选框动画期间被判 `stale_reference`，原回执写明无鼠标输入但因 AppKit 内部激活进入副作用计数而围栏输入域。查询原请求后，以原生键/鼠释放、writer idle 的只读恢复解围；原请求未重放。后续 RWI 点击状态栏/标题栏虽返回，但 AX 命中仍报参考窗被另一 Paint 窗覆盖，因此不用参考窗的 GUI 复制路径。改走**原图精确裁切 + Paint GUI 导入**。`Add Image` 的 AXPress 返回 `-25204/uncertain`，独立发现 Open 面板；对明确的 `avatar-row4-import48.png` 执行一次 AXOpen 返回 `-25205/uncertain`，独立发现面板关闭、主窗新增一个选中的 `Image` 图层且宽高48。没有重发任何不确定动作。
2. 第 4 行第一笔 RWI 画布拖拽 `dispatch=returned` 但截图仍在导入默认中心，不算成功；新观察后第二笔把图层移到第 4 行头像位。RWI `Escape` 结束编辑，`cmd+s` 后磁盘确实从旧版 SHA `1573f4…` 变为新 SHA `d1ff3d…`。细调一笔后再次保存；只读解开两层 NSKeyedArchiver 得 **131 对象 / 8 Image**，新增第130层内部自下而上 bounds `[476,292,48,48]`、嵌入PNG 48×48。最终阶段文件 [`paint-v14-20260925-avatar-row4.paint`](../../outputs/paint-v14-source/paint-v14-20260925-avatar-row4.paint) 为 453257 bytes / SHA-256 `aa8401c3d680f9578e82be248ef1dd4923fc8a9326821b9f79fc6b91a70677d9`，与工作源字节一致。原像素 RWI 截图：[第3/4行](../../outputs/validation/2026-09-25-v14-row4/native-row3-row4.png)，450×420 / SHA-256 `09cd4640600ae70e2ba0779698ed00872b95efa190a5d10c33f2252f43520cc5`。
3. **第 5 行。**参考图裁出第5行头像，并由 RWI 再次在实际 Paint GUI 经 Open 面板导入。AXOpen 同样返回非零不确定状态，但独立发现面板关闭、新 Image 出现；唯一一笔明确的新拖拽后实际落在第5行，RWI `Escape`、`cmd+s` 后磁盘另变。只读归档为 **132 对象 / 9 Image**，新增第131层 bounds `[478,208,48,48]`，嵌入 PNG48×48。阶段文件 [`paint-v14-20260925-avatar-row5.paint`](../../outputs/paint-v14-source/paint-v14-20260925-avatar-row5.paint) 为 460495 bytes / SHA-256 `6a3366a2c62154c3cf32ea365a5d8c5e69aa652321bd8a9fe0f5641f9489d17f`。实际 RWI 原像素截图：[第3–5行](../../outputs/validation/2026-09-25-v14-row5/native-row3-row5.png)，450×480 / SHA-256 `5ca9da42f87e70a2aae99f6856ab5ccf9ed50aec51ea8a70d245c65248660561`。
4. **第 6 行。**参考图裁出深色 MacApp 头像，RWI GUI 导入、拖至第6行、结束编辑并保存；非零 AXOpen 依然只执行一次，由新图层与落盘归档独立验证实际效果。最终 **133 对象 / 10 Image**，新增第132层 bounds `[478,127,48,48]`、PNG48×48。当前工作源与新阶段快照 [`paint-v14-20260925-avatar-row6.paint`](../../outputs/paint-v14-source/paint-v14-20260925-avatar-row6.paint) 同为 466241 bytes / SHA-256 `b804a35ad576296a170e07fb29eb72bc24345f6a51f20f5a74f73b8f9449c652`。实际 RWI 原像素截图：[第3–6行](../../outputs/validation/2026-09-25-v14-row6/native-row3-row6.png)，450×500 / SHA-256 `484a50d2c0def817cea93262b4c652337e680b3032598a9c2ade7cb6fddb5323`。

## 质量边界与下一步

- 三个头像是正式 `.paint` 中独立 Image，旧分层快照未覆盖；但**还不是 1:1**。原底图的酒红色头像占位圆弧在第3–6行仍局部露出，第6行尤其可见顶部弧线。曾对第4行弧线发一次 RWI 精确点击，图层未显示为单独被选中的椭圆；没有基于同色/位置猜测而删除对象。需单独核实这些弧线是底图像素还是矢量层，再设计可编辑遮盖或调整图层顺序。头像本身位置也应与参考再做原像素差异校准（第4行 x476，第5/6行 x478）。
- 保存文件保留图层与分阶段源，但 NSKeyedArchiver 中未证实逐步 Undo 历史随 `.paint` 持久化；不能向用户承诺跨重启的操作级撤销。参考图窗口留在虚拟屏，正式文档已保存，未触碰 Windows VM。
- 本轮没有产品 Rust 代码变更、部署或 Git 提交/推送；上述是当前已部署 RWI Plugin 的真实 Host 操作验收，不用 terminal 文件加工冒充 GUI 导入成功。
