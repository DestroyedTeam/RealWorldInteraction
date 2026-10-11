# 2026-10-09 动态小地图像素护栏与同帧评估修复

## 当前结论

本轮未放宽任何像素阈值、未采用red_marker_cluster、未给游戏发输入，未重启生产Agent/MCP。
已实现并验证诊断分区、同时列出全部失败预算、同一fresh-scene拒绝路径避免重复PNG比较。签名候选已准备；新接力占用后，部署/真正游戏目标验收未执行。原坐标尚不能宣称兼容性问题已解决。

## 按实际顺序的观察

1. 只读接收指定 NAVIGATION_NATIVE_GATE_20261009.json：原独占拒绝46changed/total3547/hit312/stable78/center0，native_input_dispatched=false；focus/layout/native_epoch/scope/mutation_context未变。这不是有人干预的证据，也不是整游戏发生46格变化。
2. 96×96原生像素patch分16×16格，每格6×6；中心12×12/命中48×48都为原图像素。本机2×对应桌面48×48 patch、24×24命中，1600预览大小不修改保护范围。原聚合没有位置分区，不足以判断46格究竟位于命中/外围；没有过去实际before/after PNG，不能从单crop推定。
3. 本会话loaded RWI验证Agent PID66480/instance396fdd62…、四权限true；fresh exact-PID83363/cgwindow9181/epoch不变，independent attach，仅两次只读snapshot和original artifact，不替换原desktop绑定、Live或另一任务上下文。
4. 两幅新的原始3600×2338 PNG真实保存并校验artifact SHA。Rust本体的显式ignored离线pair evaluator得到：原坐标这次33changed=hit12+context21，hit984>384、center14，同时两个预算失败；同图下方/右侧候选点各项delta0。这是新时刻的两帧对照，不是复现先前46格，更不是准入ticket、目标成功或“换点已经修复原点”。
5. 新占用通知后只读ps确认relay PID15346/parent59777仍运行；没有signal/stop-file/关窗/第二worker，未独立声称flock状态。仅detach自己的临时观察target。游戏输入0，游戏仍供现有接力使用。

## 实现与验证

- PixelAnchor::Comparison新增 hit_changed_cells/context_changed_cells/context_trimmed_changed_cells/stable_context_changed_cells 和 failed_checks；旧reason优先级、matched、MAX_CHANGED_CELLS32、hit/stable384、center16与红点规则不变。
- 精确46/3547/312/78/0回归是合成格特征，不称真实原PNG；5诊断test BASELINE101→MODIFIED0→主/补充rollback101→重施0，before原SHA一致。
- Core evaluate_grounding保留binding→element/content→marker/local OR短路；Engine.act失败消息明确实际pixel reason，不再误称Focus/layout都改变。比较值只用于同一fresh scene，不跨native batch缓存、不接收外部ticket、不续期。
- Native validate_batch_scene复用同一检查的local/red metrics。pixel identity或binding结构变化先拒绝并标skip原因；原semantic、readiness/blocked、native窗口/AX hit、permissions、lease/cancel、durable no-replay均保留。
- 同一失败输入：BASELINE同scene比较2次/测试exit101→MODIFIED1次/exit0→主ROLLBACK恢复原hash后2次/exit101→重施1次/exit0；没有输入reservation/dispatch。Native同命令3pass2fail/101→5pass/0→rollback101→重施0。这是离线比较次数证明，不是整步延迟减半。
- 最终fmt、workspace **642passed/11ignored/41suites**、all-target/all-features locked offline Clippy -D warnings、git diff --check、release Agent均exit0。operator文件pair ignored test本轮明确执行1pass，不能把原本11ignored冒充运行；阶段错误E0609/E0277及修正均保留原流。
- 同“RWI Local Development”叶证书及DR签名候选，strict requirement验证0；签名执行文件SHA `4c4d08903a4981b5c0b8082e72ff77ac75f961d52b37e458b00820238e5bb258`。没有运行/安装候选、没有新权限、未读/导出私钥；当前安装Agent SHA79b38f98…、MCP ab74d313…未变。

## 实际原图（只读采样1→2，未编辑）

![原图采样1](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/native-minimap/before-pair-0.png>)

![原图采样2](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/native-minimap/before-pair-1.png>)

## 下一操作

取得新的独占交接并验证input无并发/不确定记录后，沿用已成功的稳定签名部署事务安装本候选；fresh绑定自有fixture先验新诊断/清理，再在授权游戏窗口以默认Pixel护栏验证小地图展开目标，独立读真实完整地图。原点若仍拒绝继续定位，不换red模式、不提高阈值、不重放旧request。对可替换入口的测试只能记替代入口结果。

上轮typed hover Host表加载/GUI接收尚未完成，连同occluded/offscreen/多框架/耗时/并发回收/Live/多窗完整计划保留，本bug补充只重排优先级。

## 证据入口

完整native工具结果、原像素及映射：
/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/native-minimap

最终命令流/回滚结果/签名候选：
/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/final-dynamic-guard

继续沿用10/06主四角色；补充记录不替代原始baseline与三态行为。
