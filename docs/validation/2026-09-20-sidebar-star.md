# V14 侧栏收藏星形轮廓

## 实际过程
1. 尝试原生Shape菜单；CG窗口可见，但独立AX窗口解析失败，主窗AX菜单没有可读形状项目。CUA同样未显示菜单内部项目。没有猜测菜单坐标；仅由CUA Cancel关闭菜单。
2. 通过RWI添加 #D9D9DA 无边框Rectangle，覆盖收藏位置的红色占位圈。
3. 使用RWI Pen、2px线宽、一次11路点拖动绘制闭合星形。Smooth开/关各测试一次；两者原生细节截图都未保留期望凹角，不能只凭returned认定成功。两次Sketch均明确Undo，未保存。
4. 改为10条独立原生Line：每笔前新鲜观察、选择Line并检查AX值=1、确认映射不变；单笔出现错误/uncertain立即停止，不重放。独立截图核对完整轮廓后保存。

![保存后的真实窗口](../../outputs/validation/2026-09-20-sidebar-star/saved.png)

![原始像素局部](../../outputs/validation/2026-09-20-sidebar-star/detail.png)

## 源文件与可编辑性
通过RWI cmd+s保存工作源，并新增outputs/paint-v14-source/paint-v14-20260920-sidebar-star.paint；旧快照全部保留。只读解析确认128对象：5图片、27矩形、75线、11椭圆、7圆角矩形、3文字；无DrawSketch残留。新增正好1矩形+10线。

423768bytes，SHA256 3743a18e1d6379c9db029e58593d4598614f33acd481b49fac16b2912abcbd02。未合并栅格化，未关闭文档/清理撤销栈；跨重启Undo仍未验证。源文件manifest同步记录。

## 未完成项
星形为角较锐的轮廓，原图圆角及内部小细节尚未1:1一致；其余侧栏图标、头像、整体比例仍待完成。单次Pen路径丢失细节的原因未确定，不能归因于Smooth一个开关，也不能宣称复杂多段拖动在Paint完全通过。形状菜单/独立popover观察仍是RWI缺口。本轮仅GUI及文档推进，未修改产品代码、未提交/推送。
