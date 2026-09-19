# Paint v14 捕获候选修正

当前任务已通过Plugin真实调用恢复Paint S观察。发现Open面板被绑定为880×448pt，但返回图像包含父窗口画面；现有scene像素映射不能据此当作已校准。新增macOS26截图专用接口与显式sourceRect，排除子窗口自动扩展，保持过滤器PID/窗口/边界校验及异步回调生命周期检查。

来源：[Apple SCScreenshotConfiguration.sourceRect](https://developer.apple.com/documentation/screencapturekit/scscreenshotconfiguration/sourcerect)、[includeChildWindows](https://developer.apple.com/documentation/screencapturekit/scscreenshotconfiguration/includechildwindows)，以及本机Xcode macOS SDK的ScreenCaptureKit/SCScreenshotManager.h。

新增离线原生配置测试验证负坐标源区域、目标尺寸、禁用光标与子窗口。全workspace 436 passed / 10 ignored，fmt与全特性Clippy通过。以上仅证明离线回归，不证明实时图像已正确。

已部署候选，旧app完整备份位于work/paint-v14-20260919/previous-agent-bundle（去掉.app后缀防止重复注册）。由于临时签名cdhash改变，macOS权限需重新确认；当前系统设置停在Touch ID/密码提示。权限恢复后的现场验证与头像导入仍待执行。
