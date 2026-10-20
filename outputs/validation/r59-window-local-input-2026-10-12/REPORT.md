# R59：跨出显示器边界的窗口，实际后台输入验收

日期：2026-10-12（Asia/Shanghai）。存储合并后实际根为`/Volumes/ChatGPT/Real World Interaction`，两个旧路径仍指同一对象；本任务只复验158受管源hash，没有改迁移记录或创建项目卷。

## 同一原生目标旧／新对照

自有Receiver129：PID15825、CG43580、process_start=15825:1791775390:877867，位置一直[3225,504,660,552]；Virtual4右缘3720，165pt在屏幕外，catalog.display=null。

| 检查 | 旧Agent68669 | 新Agent18993 |
|---|---|---|
| 原图 | 已有1320×1104 | 仍有1320×1104 |
| mutation_ready/input_reference_available | false/false | true/true |
| 输入affine | null | [0.5,0,0,0.5,3225,504] |
| AX Safe一次 | 未发送旧输入 | 实际Safe count=1 |
| 1秒后台canvas drag | 未发送旧输入 | 1down/16move/1up，completed1/orphan0，蓝线 |
| 最后Close | 旧实例不试 | fresh AXClose一次，closed/expiry0/进程exit0 |

`window_mapping_ready`将“完整落在一个monitor”与“窗口本身的点／像素映射”分开：精确on-screen CGWindow、AX矩形与完整SCK图片已有独立核对。新helper保留有效有限几何、实际active display交集和受影响display不旋转；display_assignment仍null，不假称它完整属于某屏。完整离屏、hidden/minimized discovery仍未放开。

native input仍核对UID/security session/process birth、exact window/object、geometry/layout/content、modal/enabled/actions、lease和durable no-replay。没有删除这些原生保护，不借独立capture_visual的变换作为输入reference。

## 实际时间顺序图片

1. 同一窗口旧普通snapshot可读但没有输入mapping。

![旧snapshot](01-old-partial-window-readonly.png)

2. 新普通snapshot生成自身正常input frame/map；不是对只读图片强制授权。

![新input-ready](02-new-window-local-input-ready.png)

3. 一次显式AXPress后独立读回Safe count1。

![AX count1](03-background-ax-safe-count-one.png)

4. 一条新typed drag后原生NSResponder计数与实际蓝线。

![后台蓝线](04-background-canvas-blue-line.png)

18个真实鼠标回调均记录app inactive、window nonkey、frontmostPID97732，且window_number=43580。它是回调采样，不是连续前台状态或物理硬件来源证明；AXPress是语义动作，不能冒充鼠标点击。

## 未成功的原生动作保留

屏幕外ScrollArea自己advertise AXScrollDownByPage，但一次AXUIElementPerformAction返回-25205：dispatch=uncertain、effect unknown、held_input_resources=false。独立scroll events0、clip_origin[0,0]；原durable记录仍不确定，不升级成成功、不重放或改用pointer／其他输入来替代。

四个原请求（Safe、drag、scroll、Close）均终态、资源已释放、pending=false、release uncertainty=false、no replay；两binding退役，源实际显式关闭、expiry0、进程exit0。

## 前台系统光标边界

foreground_move::guard新增具体point检查：要求该点唯一命中一个active且unrotated display；无命中、重叠/镜像歧义、旋转、NaN都拒绝，不clamp/warp或自动降级后台。窗口局部BG reference不是系统光标授权。纯测试通过，本轮未运行新增负向HID案例；既有R52实际前台悬停结果保留，不把两者混作全量实机完成。

## 测试、部署、回滚

新window-local合同基线真实失败101，修正后同命令2pass/0。首次workspace因旧wiring字符串仍期待两个guard参数而失败101（partial490/1/9/16）；纠正测试到三个参数后重新跑完整11门exit0：workspace765/0/11/42、Receiver66、Live UI7、fmt、严格全targets/features Clippy-Dwarnings、release和架构检查通过。不将失败partial结果当完成总数。

新Agent18993 / 73001bbe-91f1-4177-9114-7f45a3b236f1，同证书/DR原子部署，signed SHA d44ee05a5ed5d84e99e3b44ada745110f64f3aa06f0d09168abbb4866380d7a7。四权限、MCP/CA/profile/config不变；无重启或人工授权请求。

源配置BASELINE false1→MODIFIED true0→独立copy ROLLBACK false1→REAPPLIED true0；七份pristine hash一致。不是生产回滚。仍沿用四角色/158源/原97基线。

[Apple SCContentFilter](https://developer.apple.com/documentation/screencapturekit/sccontentfilter)提供独立window filter、pointPixelScale与contentRect；[CGEvent位置](https://developer.apple.com/documentation/coregraphics/cgevent/location?language=objc)采用global display coordinates。这里用前者建立正常窗口映射，并将真正HID系统指针点的限制另行保留；API说明不等于任意客户端效果，以上独立oracle才是本次接受证据。

## 后续

F01–F16完整目标继续。全离屏/最小化/另一个Space、mixed-DPI更多布局、屏幕外synthetic效果、native foreground新增负向案例、后台tracking/tooltip、foreign-provider菜单、跨应用DnD仍未完整接受。旧R47 CmdTab、R48 Host v2 Live、R56 sheet/面板、R57诊断、R58独立只读通道结果保留。
