# Paint S v14 接续

前置：在 Host 当前工具目录中发现 Real World Interaction Plugin 的 MCP。当前任务尚无该工具面，不以终端旁路 GUI 证明插件验收。

历史基线（尚未本轮复验）：`docs/architecture/macos-coordinate-model.md` §31。v13 卡片为独立 Image 图层，bbox x831..996/y110..300，历史报告最大偏差1px。当前应用文档/窗口/图层仍需重新发现，不能沿用旧PID、句柄、frame或坐标映射。

下一步：
1. Plugin `computer.targets` → 选择 platform=macos 的用户目标 → 用新发现结果定位 Paint S 文档。
2. 观察当前画布及图层列表，确认是否仍为 v13，并保存只读基线证据。
3. 核对源图与当前底图：第1–6行头像应各自成为独立 Image 图层；第7–8行历史记录称无头像，须以源图复核。避免把新头像叠在原底图头像上后误认为已拆层。
4. 复用已有合法素材：`work/replica-verify-20260915/assets/avatar-row1-place48.png` 和 `avatar-row1-6x.png`；其他各行资产/尺寸仍需核对，不凭历史摘要捏造。
5. 优先用应用 `Add Image` 导入、`Object > Transform…` 数值定位。每层命名/位置/可独立选择必须在当前原生GUI中核验。
6. 保留可编辑文档与图层；导出v14并按时间顺序记录真实截图、参考对比及误差。试用版水印与独立图层保存能力单独报告。

用户约束：可复刻形状/文字优先绘制或渲染；不可复刻的照片/头像可贴图，但必须独立保留图层。不得把离线合成的整幅图片当成应用内独立图层成果。
