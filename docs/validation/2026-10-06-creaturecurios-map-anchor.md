# CreatureCurios 动态地图像素锚点复查（只读）

日期：2026-10-06。来源任务交接报告 CreatureCurios「青芽原野」7 个地图目标测试；本任务只做源码审查与分配窗口的只读复核，不向游戏发送输入。

## 交接记录与本轮证据边界

- 来源会话报告两次地图红点点击（`qingya-map-marker-top-daerhou-20261006-01`、`-02`）均以 `stale_reference` 在派发前结束；交接称 `content_changed=true`、focus/layout/native_epoch 未变化、`native_input_dispatched=false`。该会话的 `read_thread` 本轮返回 0 个 turn，当前仓库也无这两个 request 的原始 QA receipt/截图，因此这是交接记录，不是本轮独立读取的调用回执。
- 当前 RWI 只读选中 `mac-host`，确认服务 `healthy/available`。按交接给出的精确 PID 46833 查询，发现唯一 on-screen 窗口 `cgwindow:22944` / `CreatureCurios`，范围 `[0,0,1800,1169]`，位于内建屏；无焦点切换、窗口移动或键鼠输入。
- 第一次 bind 因 10 秒 discover catalog 过期而返回 `catalog_expired`；回执明确 `native_dispatch_entered=false`、`input_domain_fenced=false`、资源已释放。立即重新 discover 后，对同一 PID/window/native_epoch 的 `relation=initial` 绑定成功，也未激活或输入。
- 只读 snapshot 用时 2572 ms（observe 1912 ms、image 659 ms），原图 3600×2338，预览 2048×1330；`visual_only=false`、`input_reference_available=true`、`mutation_ready=true`、`blocked=false`，但 AX 控件数为 0。保存图像的原分辨率地图角落 crop 显示当前是 HUD 加小地图/红点，中央还打开“强化成功”面板；这**不是**交接中地图选择界面的同状态证据，不能据此判断七个目标能否点击。

## 源码判断

当前 `crates/rwi-core/src/pixel_anchor.rs` 已不是整窗像素相等：96×96 局部纹理按 16×16、每格 6×6 像素比较；中心 12×12 有严格最大差值阈值，周围 48×48 hit region 有独立差值预算，外围仅容许有限动效格，剩余稳定上下文仍有限额。Mac 的像素锚点只给 exact selected window 的非 semantic-only surface；派发仍需 exact window/PID、原生 hit/object 证明、frame/transform、lease、blocked/modal 与 durable no-replay 检查。

不能仅因窗口/焦点/layout 不变就忽略中心像素变化：它也可能表示红点移动、被遮挡、标注消失或坐标落在了另一对象上。当前交接没有 `central_max_delta`、`hit_region_delta`、每格变化或失败前后的局部图，因此无法判断这次是外围动画预算过严，还是目标 patch 本身不稳定。交接称用户授权来源会话操作7个地图目标；即使如此，授权确定的是业务范围，不会自动证明变化后的坐标仍指向同一红点。

离线复跑 `rtk cargo test -p rwi-core pixel_anchor --locked --offline`：8 passed / 148 filtered。该组测试验证既有“外围小动画可容忍、主体移动/中心变化/宽域漂移仍拒绝”，不是当前 CreatureCurios 地图实机验收。

## 决策与后续实验

- **本轮不调宽全局阈值，不加“授权即放行”分支，也不重放两个 request。** 否则所有 Mac 自绘窗口都可能把过期坐标当作地图目标。
- 如果要做专用 map-marker 适配，先需同一已分配窗口在动画前后的原尺寸目标 crop 与逐次局部锚点诊断，确认红点本身稳定且可与邻近标记区分。候选应只比对被识别的 marker 特征，窗口/PID/process/native epoch/显示映射/命中窗口与 marker 位置仍须精确；marker 缺失、移动、相似标记歧义、modal/layout 改变仍拒绝。离线负例覆盖 marker 移位/消失、覆盖与重复红点；通过后才有一个新 request 的单次实机验收和独立效果读回。
- 若红点中心区域会随动画改变且找不到稳定唯一特征，安全选择是等待静止帧或寻找原生语义入口；不是取消目标级像素门。

本轮没有改 Agent/MCP、没有部署、没有点击游戏，也没有将本轮屏幕图像写入仓库。当前 Host 的成功 snapshot 仍回传默认计时字段，说明上一轮 MCP 输出精简候选尚未部署。
