# F153：后台文字接收链的原生实机验收

范围：mac-host 上专用 Rust AppKit 接收器，PID58980／cgwindow:3906，仅虚拟屏4（UUID13E4E9B0-47F2-4043-9CDC-878D5441BD41）。不是 Chrome、真实用户硬件输入或全应用兼容率。

| 顺序 | 实际动作与观察 | 结果／问题 |
|---|---|---|
| 1 | 新鲜 exact-PID 发现、独立 attach、snapshot | 原始空字段；1320×1104；仅选中自有窗口，无激活 |
| 2 | 一次 `focus_element`，随后新 snapshot | AX 本地焦点 ready；仅 native_ax_focus_only，不当作应用键入效果 |
| 3 | 一次新 `type_text`，载荷 `后🙂𝄞 é👩‍💻` | 新 AX 值及自有状态均完整13/13 UTF16，9次文字变化；回执自身仍 goal_verified=false |
| 4 | 新 public NSTextView 诊断 | keyDown／interpret／insertText／keyUp 各有进入与退出，共72条；9组完整、缓冲丢弃0／越界忽略0／observer panic0 |
| 5 | 一次新快照与 AXPress Close | 命令退出0；自有状态closed无error、PID不存在、新发现空、解绑；3个原请求均终态／释放／禁重放 |

## 真实截图（本地验收附件，不随 Git 发布原始 PNG）

![01 原空字段](01-before-empty.png)

![02 一次后台文字输入后](02-after-background-unicode.png)

两图是实际 RWI MCP 响应的原 PNG 字节，未重新编码，尺寸1320×1104；hash见本地 `IMAGE_MANIFEST.json`。

## 边界与保留问题

- 9次 keyDown 与 insertText 的 raw UTF16 串接均为完整载荷；9次 keyUp 的 characters 实际均为 `a`，原值保留。此接收器仍完整插入，不能仅由 keyUp 字符差异推出 Chrome 丢字根因。
- 所有已录回调的 window_key/main=false、firstResponder匹配；OS前后台完整元组在开始／结束相同（PID767/window3873），环境变化记录均未见目标成为OS前台。但AppKit application_active从false变true，**不宣称应用内部激活状态完全不变**；采样不是原子或无遗漏的连续系统追踪。
- field_editor_requests包含诊断的currentEditor getter及框架查询，不是325次点击、焦点写入或键盘尝试。[Apple currentEditor 文档](https://developer.apple.com/documentation/appkit/nscontrol/currenteditor%28%29)与[按键解释文档](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/HandlingKeyEvents/HandlingKeyEvents.html)用于接口设计，不替代本轮Native记录。
- RTK包装命令PID58979与原生接收器PID58980分别保留；观察到的是包装命令退出0，未单独读取原生子进程waitpid。首个核验器把两PID误作相同而退出1，修正核验器新命令退出0，未重放GUI输入；环境记录初读对象形状错误也保留。
- 仅新增默认关闭的测试夹具诊断及dev-only Foundation NSRange特性；生产Agent47164／8c2d9f8f、MCP安装、四权限、签名／CA／配对／Host设置未变。复用启动器的历史LIVE文件名不是调用过Live，本轮没有Live start。
- Chrome155后台原13→5负例未修；Live v5真实Host像素／模式未知、任务导航未停采仍未通过。前后台冲突、模态／系统／隐藏／层叠、普通冷悬停和完整工具／分范围性能验收继续；全目标未完成。

离线可逆性：夹具137→142→137→142；开关拒绝1→接受0→拒绝1→接受0，check-only无GUI；Source485原hash及absences在独立副本恢复、Source487重新应用，原四角色390项全量重建与防误回滚通过。workspace1384通过／11忽略与显式example142是不同测试范围，Node组件48；fmt／workspace／all-targets/all-features clippy-Dwarnings／架构均退出0。
