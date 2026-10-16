# 独立绘图结果后续：像素预检与客户端就绪拦截

## 输入依据

独立任务 `01a096c8-7e2b-7fc3-8008-7d9886c4e051` 已完成完整 Nemessix 图标与
GUI 保存，重开被焦点检查拒绝。用户认可成图效果。复核该任务 CSV：386 次
嵌套调用、90 次 observe；observe 中位 962.955ms，act/act_checked 中位
3013.041ms，与报告一致。23 条轨迹成功；不据此宣布完整可替代性验收。
报告整轮 2582.033s、绘图 767.540s，工具外间隔含构图/编排/调度/人工等待，
不是纯思考。原报告保留在该独立任务的 outputs，不修改它的历史结果。

## 本次实现

- 普通 native-element policy 的像素输入预检只读 UIA root identity，不遍历
  无关子树；保留两次原生图像/绑定复核及最终逐点 UIA/焦点/权限检查。
- 语义输入和 legacy whole-document policy 保持全树/原文档约束；纯观察仍为
  原来的完整 bounded walk。未新增业务名称、硬编码画图流程或脚本输入桥。
- 设备新增按 scope 分开的预检计时/失败计数，便于区分改进和剩余开销。
- JS `withInput` 在 `mutation_ready` 非 true、blocked 非 false 或 fence=true
  时，本地拒绝且不 claim；批内下一次新观察不就绪则停止，仍释放原 lease。
  `RcuInputNotReadyError` 保留 frame/details_ref，明确是客户端拒绝，不是
  设备已执行的回执；不自动观察、重放、恢复 fence 或扩展授权。

## 原生 Host 对照

使用同一已保存 Paint / PID 2024 / hwnd:2e0636，工具栏已选中的 Pencil。
输入通过当前 Host 原生 `computer.js` → resolve_view → claim → act → release；
不是 Shell MCP 客户端。每次均重新观察，已知几何改变就停，不在画布落笔。
各样本前留 900ms 空闲，不持 lease；此等待不计为动作调用耗时。

| 样本 | 原 agent 像素 act | 新 agent 像素 act |
|---|---:|---:|
| 1 | 1669.137ms | 439.307ms |
| 2 | 3589.013ms | 1344.096ms |
| 3 | 3585.969ms | 453.463ms |
| 中位 | 3585.969ms | 453.463ms |

均成功插入并释放，无 fence。3 个像素动作的设备计时为 root_identity 6 次、
full_tree 0 次，证明两次复核保留而非被删除。root-only 总计 1577.934ms，最大
811.314ms；保留这个慢样本，不宣称每次都稳定 450ms 或整体绘图提速同倍数。
样本小、同 app 顺序对照且有 provider 波动，不是受控跨场景 benchmark。

再以新鲜语义 Pencil ref 调用 act_checked，成功；设备 full_tree 增加 3 次，
共 4668.979ms，动作调用 4924.382ms。语义慢路径仍是下一项，不假报已优化。

本轮接手时的完整窗口 PNG 前后均为
`fb34da3bd9b3d98c8ecf12170b8d2917eb2286e3db6455b34c9ae1237b5e290f`；
512×512 原生图块前后均为
`ab04a812882dc0a16fa93ab02d78cc8aaf172627a3d7d5b66f3379503fa7ae75`。
这只证明本轮未改变接手的画稿，不把它与独立任务更早的成图 hash 混为一谈。

## JS 入口验证

- 原地更新全局 adapter 二进制，未改工具目录、profile、身份或 Codex config。
  仅调用当前 Host 的 `computer.js_reset`，不重启其它 MCP 父进程或 Codex。
- 实际 Host worker 中做**显式客户端合成负例**：真实观察 ready=true，只将
  本地返回对象的 ready 置 false；helper 返回 client_precondition。trace 只有
  observe，没有 claim/action。不是声称 Windows 此时真的拒绝了权限。
- 随后重新获取真实观察，通过原生语义 Pencil 点击成功并释放，证明就绪恢复
  不会永久锁死窗口；不是重放先前动作。

## 部署和回归

- Windows 功能版本 PID 12688 / Session 1，实例
  `a62687a4-3855-4564-a070-1518b3833659`，SHA256
  `f22523611f462031c93056fb79d18401b67ca25a0714c2f8a2c1a5d1162cefab`。
  按 exact PID/path/birth/hash 替换，保留 rollback-pixel-preflight-v1；
  配置、SSH、防火墙及启动任务定义不变。
- Mac SDK adapter SHA256
  `ad28d7249a7c5de7c34cbae54cbc8271a5f45b4c4961b65820d8dd555e6348a7`；
  原二进制已独立保留。SDK 源文件 SHA256
  `018889c3eb5e9f681e2627d1e10e61e3de2bb717528249efa05553cc6c2a97cc`。
- 默认 workspace：281 passed / 3 ignored；fmt 和 all-targets/all-features
  clippy 通过；Windows ARM64 release/cross-clippy 通过；JS 18 tests 通过。
  增加 scope 路由、图像/epoch/layout/title 约束、就绪拦截与真实 fixture RPC
  不发 claim/action 的回归。最终 all-features：368 passed / 4 ignored，38 suites。

## 独立的纯模块重构

- `pointer_feedback/native.rs` 758→467 行，窗口/Surface/放置移至
  `pointer_feedback/native/window.rs`（306 行）。
- `uia.rs` 748→584 行，输入作用域移至 `uia/input.rs`（168 行）。
- 保留函数体，机械比较仅忽略空白及模块内所需可见性；两组比较均通过。
  架构规则文件未修改，ratchet 检查 passed=true / violations=[]。
  既有允许债务仍在，不将这个结果解释为“整体架构已完成”。
- 重构后 release/cross-clippy、默认 workspace、all-features workspace 均通过。
  最终 Windows 进程 PID 13060 / Session 1，实例
  `e0057173-88fb-4b6e-9755-a015face838d`，SHA256
  `d4e9efc950605ba8e54775bdc791c114825c5f61bac438669ec060147b62e352`。
  独立 rollback 保留，配置与任务定义未变。
- 最终构建再次经当前 Host 原生 MCP 点击验证：act 322.062ms，root-only
  复核 2 次、full-tree 0 次，插入/释放正常；窗口及画布 hash 与上表不变。
  这次 smoke 不混入前后 3 样本对照，不作为额外性能倍数证据。

原始 native 回执、安装记录及日志在 `work/pixel-preflight-20260913/`，
截图、二进制、profile 和运行身份均不提交 Git。后续纯模块拆分单独提交，
不靠上调文件阈值使架构检查变绿；仍不声称正式 UIOS 或整体架构验收完成。
