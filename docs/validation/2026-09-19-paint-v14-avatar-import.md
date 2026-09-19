# Paint S v14：首个头像独立层与滚动对照

## 实际步骤（按顺序）

1. RWI Plugin 在 PID 2578 / Open cgwindow:5719 中进入 assets。AX 观察与精确查找预算扩展为 512 节点、1024 队列，保留 3 秒、16 层边界。系统只暴露当前分组内容；扩大预算不等同枚举整个目录。
2. RWI 的 AXScrollDownByPage 返回 -25205，截图没有确认滚动；滚动条按钮查找返回 semantic_target_missing，没有发送输入。没有自动重发。
3. 用户授权的 Computer Use 对照：Paint S.scroll(29, 'down', 1)，调用实测 578 ms（单次，非基准结论）。最新 AX 树由 Today 改为 Yesterday，滚动条值由 0 变为 0.7718446601941747，出现 avatar-row1-place48.png。因此这段是 CUA 辅助，不能计作纯 RWI 闭环。
4. RWI 重新观察并分页获得精确头像元素，执行唯一请求 v14-avatar-row1-import-20260919-01 / AXOpen。原生返回 -25205，保留 uncertain；没有再次点击。随后旧 Open 消失，重新发现并绑定主文档。
5. RWI 实际截图确认：左侧出现第三个 Image 图层，选中层尺寸 Width:48 Height:48，画布显示独立头像及控制柄。原截图 artifact b1ca82ec-ba40-420f-aaf6-0212283fca0a，SHA256 5296d35635d51001ffc807332a8a9a84e9adb829f5f8b5a911da31ca7f469a1e。这是有限保留期运行态引用，不保证永久可下载；截图已在任务中展示。
6. 后续定位拖动被 stale_reference 在原生投递前拒绝。头像仍停在导入位置，没有将导入写成位置验收通过。添加 grounding 各维度变化诊断，并修复 Agent 错误封装丢失 Fault.diagnostic 的问题；不放宽像素内容校验。

## 尚未完成

- RWI 文件面板滚动尚未生效；页动作仅按目标广告能力开放，不能宣称滚动支持已验收。
- 头像定位、剩余头像独立层、重复卡片底图修复与最终保存仍待完成。
- 不代表 v14 复刻通过，也不代表所有系统/第三方跨进程面板已支持。

## 后续复核

- 修正已部署，同一本地证书重签，Plugin 查询四项权限仍为 true，无新增授权。fmt、workspace offline tests（441 passed / 10 ignored）、全目标全特性 clippy 通过。
- 新观察后请求 v14-avatar-row1-place-diagnostic-20260919-03 得到 dispatch=returned（1567 ms），但独立截图未显示头像位移。诊断改动没有放宽任何校验；不能把这次返回归因为拖动问题已修复。
- Computer Use 也进行了单次定位拖动对照（538 ms），截图同样未确认头像位移。当前证据不足以归咎于 RWI 单独故障或应用限制，需继续检查坐标映射、选择控制柄及应用实际接收路径。未继续重发。
