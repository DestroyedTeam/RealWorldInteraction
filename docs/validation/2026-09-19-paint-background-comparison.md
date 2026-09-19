# Paint S：Computer Use / RWI 对照与候选修正

## 2026-09-19 实测顺序

1. 用户手动补齐授权后有序重启唯一 RWI Agent。Plugin 查询 accessibility、listen_event、post_event、screen_capture 全为 true。
2. RWI 绑定 Paint S PID 2578 的 Open（cgwindow:4568），返回 activated=false。发现 7.7ms、绑定 28.5ms、观察 774.2ms、预览 40.5ms。后台截图可取得，但顶部裁切/底部空白仍不正确，不能视为映射校准完成。首批49控件全 hidden/occluded。
3. CUA 同一 Open 面板 AX 能读出 Cancel、Open 与文件。其截图混入父窗口并带右侧空白，不能认为 CUA 在此没有截图问题。CUA Cancel + AX 复核1528ms，面板实际消失，没有修改画布。
4. RWI 主窗口观察914.1ms、预览98.2ms，首批47控件全 hidden/occluded，blocked=false。
5. CUA 选择 Line 工具并复核888ms；Line 值由0变1，Select and Move 由1变0。未绘图/移动图层。此结果证明 Paint S 接受该工具路径的控制操作，不证明 CUA 内部究竟使用 AXPress 还是 CGEvent，也不证明所有键盘/拖动场景都成功。
6. RWI 再观察928.5ms，确认 Line 配置面板出现；retained metadata 分页仅0.26ms，不需要重新截图。
7. 通过 Finder open 显式请求激活 Paint S 后观察708.3ms，首批47控件仍 hidden。未获取可靠 foreground PID 证据，因此这一项不是已完成的前台/后台 A/B，不能将其称为前台成功。

这些是少量端到端样本，不是吞吐基准，也不能直接将 CUA 的“动作+复核”与 RWI 的单次观察时间比较得出谁更快。系统设置前序测试存在同名 Add 引用歧义和坐标点击误定位；不支持“只为系统内置应用优化”的结论。

## 代码中已定位的矛盾

`native/metadata.rs` 使用 system-wide AX hit-test 构造观察可见性，而 `native/input/anchor_native.rs` 允许 PID/窗口定向后台投递。被其他应用覆盖时，前者将后台控件全部标成不可操作，后者尚未获得执行机会。这是 RWI 的实现不一致，而不是 Paint S 拒绝模拟事件的证据。

候选修正：观察改用 application-scoped AX hit-test；仍要求 CFEqual 命中遍历中确切控件，保持应用内部窗口 z-order；失败返回 unknown，而非虚构遮挡或授权。边界/模态检查保留。

[Apple AXUIElementCopyElementAtPosition 文档](https://developer.apple.com/documentation/applicationservices/1462077-axuielementcopyelementatposition) 明确 application 参数可以限制目标应用，system-wide 不限制应用。

离线 fmt、workspace locked/offline 测试437 passed / 10 ignored、全目标全特性 Clippy -D warnings 通过。单元测试只验证状态组合，不是后台原生输入验收。候选尚未部署，避免再次更换临时签名使刚恢复的授权失效。

## 时序与剩余工作

- `computer.targets` 多次约5003ms，gateway 已并发检查各设备，但等待所有结果；离线 Windows 的连接期限拖住聚合返回。已选 Mac 的后续绑定/观察无需反复 targets。后续可加入显式单设备刷新，而非静默隐藏离线设备或缩短输入超时。
- 在统一一次部署前，还需校正截图 sourceRect 与 filter 的实际坐标系；本次已观察到主窗口顶部裁切和底部空白，不能靠尺寸相等宣告正确。
- 原生输入点命中逻辑与观察命中逻辑仍需统一验证；不移除身份、模态、未知状态、lease/no-replay 防护。
- 完整 A/B 需记录真实前台 PID、同一可逆动作、AX动作和事件动作分开、多次计时，之后再验收拖动和键盘。
- v14 第1–6行独立头像图层及底图重复卡片修复未完成，本次仅关闭 Open、选择 Line，没有新增画布内容。
