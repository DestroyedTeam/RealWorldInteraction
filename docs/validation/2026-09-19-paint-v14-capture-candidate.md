# Paint v14 捕获候选修正

当前任务已通过Plugin真实调用恢复Paint S观察。发现Open面板被绑定为880×448pt，但返回图像包含父窗口画面；现有scene像素映射不能据此当作已校准。新增macOS26截图专用接口与显式sourceRect，排除子窗口自动扩展，保持过滤器PID/窗口/边界校验及异步回调生命周期检查。

来源：[Apple SCScreenshotConfiguration.sourceRect](https://developer.apple.com/documentation/screencapturekit/scscreenshotconfiguration/sourcerect)、[includeChildWindows](https://developer.apple.com/documentation/screencapturekit/scscreenshotconfiguration/includechildwindows)，以及本机Xcode macOS SDK的ScreenCaptureKit/SCScreenshotManager.h。

新增离线原生配置测试验证负坐标源区域、目标尺寸、禁用光标与子窗口。全workspace 436 passed / 10 ignored，fmt与全特性Clippy通过。以上仅证明离线回归，不证明实时图像已正确。

已部署候选，旧app完整备份位于work/paint-v14-20260919/previous-agent-bundle（去掉.app后缀防止重复注册）。由于临时签名cdhash改变，macOS权限需重新确认；当前系统设置停在Touch ID/密码提示。权限恢复后的现场验证与头像导入仍待执行。

## 后续授权恢复实测

2026-09-19：仅对当前应用开关 off/on 并重启，四项 preflight 仍为 false。使用 Apple 官方 tccutil 按 bundle ID `com.destroyedteam.real-world-interaction.agent` 分别重置 Accessibility、ListenEvent、ScreenCapture，然后通过系统设置文件面板重新添加同一已安装应用包；未修改其他应用权限，也未读写 TCC 数据库。

重新添加控制权限与输入监控并由 LaunchAgent 有序重启后，Plugin `computer.targets` 实测 accessibility=true、listen_event=true、post_event=true；screen_capture=false。屏幕录制页面存在两个无唯一标识的 Add 控件，CUA AX 引用重新解析报告歧义；坐标点击未打开添加面板，未执行任何所选其他应用的删除或开关操作。需要用户点击上方屏幕录制列表下的加号后接续。Paint S 文档未修改，捕获候选尚未完成实时验收。

依据：[Apple 定向重置授权说明](https://developer.apple.com/documentation/xcode/resetting-access-to-protected-resources-in-macos)。

## 2026-09-19 全窗口裁剪修正现场结果

在macOS27，desktopIndependentWindow过滤器已经限定单窗口；再将屏幕全局窗口rect传入SCScreenshotConfiguration.sourceRect导致再次裁剪。现改为保留native默认sourceRect（Apple文档：未设置则完整捕获输入帧），仍校验PID、窗口边界、过滤器尺寸、输出像素尺寸，排除子窗口/阴影。

重新编译、固定证书签名并部署后，Plugin绑定新Open cgwindow:5573，完整面板预览可见顶部导航、底部Cancel/Open，无旧版顶部裁切/底部白带；观察845.5ms、预览37.3ms。此次为面板图像边界的目视验证，不宣告所有显示器/窗口几何校准通过。控件AX命中仍hidden/occluded，不能据图像正常推断输入正常。

本次用户关掉旧面板后，使用新观察发出`v14-open-after-user-close-20260919-01`，普通click回执487.1ms；紧随discover尚无Open，下一次只读discover确认面板出现。没有重复点击，证明异步UI不能用第一次空结果断言失败。新JS回执逻辑未再把system_accepted=null伪造为operation_failed。

fmt、438 Rust tests passed /10 ignored、全目标全特性Clippy通过。候选更新备份在work/v14-capture-full-frame/previous-bundle。

参考：https://developer.apple.com/documentation/screencapturekit/scscreenshotconfiguration/sourcerect
