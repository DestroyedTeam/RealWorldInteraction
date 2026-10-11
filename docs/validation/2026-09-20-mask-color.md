# 侧栏 # 遮盖色修整与弹窗对照

## 结果
将既有矩形填色由 #F2F2F2 改为 #D9D9DA。真实预览像素核对：外侧(308,290)/(327,290)及遮盖内部空白(313,284)/(323,294)均为RGB(217,217,218)，白色补丁色差消除；四条线保留。

![保存后](../../outputs/validation/2026-09-20-mask-color/saved.png)

原生源文件已通过RWI cmd+s保存，并新建不覆盖旧版的交付快照：outputs/paint-v14-source/paint-v14-20260920-mask-color.paint。422140 bytes，SHA256 ab571735b04841b7b2914d48e96b5a68a649a45158054da8fef223ab3102d35a。只读归档核对仍117绘图对象，无栅格化。撤销栈持久化仍未验证。

## 操作与边界
- RWI单次AXPress以及CUA单次点击/Show color panel，均曾未观察到弹窗出现；不把返回回执当成功。
- 参考官方说明，CUA双击后出现稳定颜色popover。此前做过明确告知的Raise前台对照，因此不能宣称完全后台、纯RWI成功。
- 对popover里的Input Color Manually，RWI显式AXPress返回uncertain/-25204，但独立discover确认外接屏上的Input Color对话框已打开。查询原请求确认held_input_resources=false、fence=false，未重放动作。
- 新鲜选择真实cgwindow:7754（非推断owner链），RWI切换Hexadecimal、点击空字段、输入#d9d9da，AX值确认匹配，点击OK；主窗口填色AX值确认0.85098/0.85098/0.854902。输入255ms、确认202ms。
- 官方说明：颜色按钮双击可固定面板，Input Color Manually可输入RGB/HEX。https://static.trybestapps.com/paint/topics/choose-color.html
- Paint的对象就是图层，官方FAQ：https://static.trybestapps.com/paint/faq.html

本次是CUA+RWI混合流程；RWI原生owner-chain和独立popover截图仍待完善。整体V14未完成。本轮没有修改/绕过权限或清除操作历史。
