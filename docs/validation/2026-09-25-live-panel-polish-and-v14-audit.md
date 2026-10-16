# RWI Live 浮层瘦身、隐藏/关闭与 Paint V14 视觉核对（2026-09-25）

## 用户反馈与本轮边界

用户指出 Live 的磨砂黑边/深色空白过重，并希望有明确的**隐藏**和**关闭**两种操作；同时要求 V14 的颜色、图标、文字逐项核对。正式 Paint S 源仍位于 BetterDisplay 虚拟屏 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`，所有本轮截图与 Live 来源均绑定该屏的 PID14404 主文档；未将用户内建物理屏作为实机预览靶场。物理屏与虚拟屏共用相同的 Live 状态机和 AppKit Presenter，但**不能因此声称已在物理屏完成视觉验收**。

Computer Use 可读 Paint S 原窗口（返回2868×1320原始画面），也可读 RWI Agent 的 Live 系统浮层；尝试读取 Codex 应用自身时被 Computer Use 明确拒绝，因此这轮能对照实际 Paint/本机 RWI 浮层与用户描述，**不能**宣称直接看到了 Codex 内置 Live 的私有实现或证明两者完全等同。

## 按时间顺序的实际结果

1. **复现旧版外观。**旧版已部署 Live 在该源、该虚拟屏、`max_edge=2048` 下原生读回 `source.image_size=[2048,942]`、`bounds=[2368,455,1040,577]`。Computer Use 实际截图显示顶端和图像四周有大面积近黑底；代码核对旧 Presenter 使用固定 2:1 图像槽高度、0.97 不透明黑色 CALayer、强制 Dark Aqua 和窗口阴影。源图真实宽高比2048/942≈2.17，固定2:1槽自身制造了纵向留边；不是原图黑边。旧 Live 已用 RWI Stop 清空且读回窗口退出、worker 完成后才部署。
2. **实现。**Presenter 依据实际 `source.image_size` 算预览高度，2048×942 的1040pt宽面板由577pt缩至526pt，缩去51pt空白；第一帧到来前不建立错误比例的运行中面板，源尺寸变化时重建。把黑色CALayer换成原生 `NSVisualEffectView` 的语义 Popover 材质/behind-window混合和浅色外观，关掉窗口阴影；保留圆角与可读的系统文字色。Apple 对 [Visual Effect View](https://developer.apple.com/documentation/appkit/nsvisualeffectview) 的说明支持用语义材质和 behind-window 混合呈现浮动层，而不是自行模拟深色透明背景。
3. **交互。**新增 `LiveAction::Hide` / Hidden 状态；原生按钮从“暂停”改为“隐藏”，点后取消在途/待呈现帧、清图并 `orderOut`，会话本身仍可 `computer.live resume` 恢复。原有 API `pause` 继续保留显示已清空占位提示的兼容行为；“关闭”仍 Stop，释放会话。隐藏时来源重绑或来源错误不自动重新显示；Host 面板切走/遮盖同样隐藏，但不会覆写用户手动 Hidden。未添加用户不要的宠物投放功能。
4. **门禁与部署。**`cargo fmt --all -- --check`、`cargo test --workspace --locked --offline` **517 passed / 10 ignored**、全 targets/features Clippy `-D warnings` 和 `git diff --check` 通过；release 构建成功。使用原 `RWI Local Development` 证书签署并原子替换 Agent，designated requirement 仍是同一 bundle ID 和证书 SHA-1 `24B346A95EC8F450F7D8433A2E1876568DBCB2F8`；新 Agent PID40775 / instance `4391ae85-7406-435d-8053-be6e702abc79`，辅助功能、屏幕录制、输入监控、发送事件均为 true，未重新授权。已更新 MCP 二进制（SHA-256 `abcd5d6c832471bf296ee03075101f2a8093d70f585b8557d6ecc15462661c05`）与 Codex Plugin `0.2.0+codex.20260925131605`；本任务先前加载的旧工具 schema 仍需 Host 新任务/重载才会直接显示 `hide` 枚举，但**原生按钮已经通过新 Agent 生效**。未改 TLS 配对/profile/journal 或其他插件。
5. **真实 Host 复验。**新 Agent 对同一 Paint 源、同一虚拟屏返回 `image_size=[2048,942]`、Live 原生面板 `bounds=[2368,506,1040,526]`、`presenter_visible=true`、`presenter_image_present=true`、`last_error=null`。Computer Use 再观察 RWI Agent AX 树，控件确为“隐藏/关闭”，截图显示近黑大边框已消失、图像紧贴浅色小边。随后 RWI 显式绑定这个 Agent 窗口只读 `computer.capture_visual(max_edge=1024)`，且 Live 来源仍是 Paint 主窗；保存的[真实 RWI 浮层截图](../../outputs/validation/2026-09-25-live-panel-polish/after-native-rwi-1024.png) 为1024×517 / SHA-256 `5795d0bceebc56962f610edaa672730934d3c47db987cb77bcce616b0613ea13`。该捕获尝试2048时诚实返回 `visual_output_budget`，降至1024才成功，不能把这张证据称为原生全分辨率截图；Paint 细字仍可用 `computer.image` 取原像素局部。
6. **隐藏与恢复。**Computer Use 对“隐藏”发一次点击；该 UI 调用最后报 `timeoutReached`，所以不凭其回执宣布成功，也**没有重放**。独立 RWI 状态回读为 `status=hidden`、`user_hidden=true`、`presenter_visible=false`、`presenter_image_present=false`、窗口服务器 `on_screen=false/settled=true`；对 Agent PID 的新鲜 discover 也为0个窗口。随后 RWI `resume` 读回 `running`、图像/窗口可见、同一 `[2368,506,1040,526]`、settled=true。
7. **关闭。**Computer Use 对恢复后的“关闭”按钮点击一次，RWI 独立状态为 `stopped` / `stop_reason=requested`，原生读回 `on_screen=false/settled=true`，随后 `worker_done=true`、`native_call_pending=false`。另起一次短 Live 为截图取证后也由 RWI Stop 并重新绑定正式 Paint 主窗；没有遗留常驻 Live。

## V14 同尺寸局部对照与下一优先

RWI 原生窗口局部 [当前画面420×680](../../outputs/validation/2026-09-25-v14-visual-audit/actual-sidebar-native-420x680.png)，SHA-256 `3b5cc6177643f3cc202e32ddef0a83ed3e22e9c4e4ce68041cc6d6067474814a`；参考原图按窗口已知缩放/位置裁切成[对照420×680](../../outputs/validation/2026-09-25-v14-visual-audit/reference-sidebar-mapped-420x680.png)，SHA-256 `4213cc05adaa964f64e6227902a90043b22f947841ee08f65daaf009378d1a7b`。这是**近似几何配准的视觉对照**，不是已证明的逐像素完美注册或单一误差分数。

- 参考左侧栏平坦背景样本 `(20,210)` 为 `#F5F5F5`，当前为 `#E0E0E1`，明显偏暗；参考首枚消息图标在当次粗采样中记作 `#1697FF`，当前同点仅为灰色占位底。**后续勘误**：原参考 PNG 与已映射局部重新读像素，蓝色主体占优值为 `#1A83FE`，以[后续实测](2026-09-25-popup-binding-and-blue-base.md)为准。其余未完成图标仍为酒红圆环/缺省形状。下一次应先确定可编辑底色遮盖层的图层顺序与取色，再逐个重绘图标，不把整张参考截图烤入画布。
- 对话列表平坦区域 `(150,340)` 双方均为 `#EBEBEB`；选中行平坦区域 `(200,430)` 参考 `#E4E4E4`、当前 `#E0E0E0`，位置差异不能靠整体调色替代。选中头像外旧酒红大椭圆、横线仍在底图，应局部遮盖并保持头像独立 Image。
- 参考列表文字笔画更清晰、且当前部分行有重叠/酒红横划；不能把原底图里的文字当作已完成可编辑 Text。需要按参考逐行核对字形、字号、基线、颜色和右侧时间，再分层改绘。Paint S 试用版水印是应用渲染，不计入画稿设计。

本轮对正式 `.paint` **仅观察、未编辑**；当前工作源与第8行阶段快照仍同为477415 bytes、SHA-256 `3f4c26dc9517edc512c832df417965ef2124825a2246bd0269dbc76d092d6423`。无 Git 提交/推送，避免触发私有仓库 Actions。
