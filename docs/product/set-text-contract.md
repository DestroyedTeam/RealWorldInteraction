# 显式整值文本替换（候选）

`computer.set_text({target, frame, element, request_id, text})` 只替换当前完整普通文本 AXValue；空串清空，最多4096 UTF-8字节。不是键盘输入、追加、光标或选区替换，也不是 `type_text` 的自动fallback。内部短lease和durable once，同一request不得重放。

仅macOS原生已观察/retained的普通AXTextField/AXTextArea，已知无specialized subrole、非secure、CFString全文值、AXValue fresh writable及原身份/窗口/provider/context/应用custody guard成立；ComboBox/date/number等专用role不支持，不尝试其它setter。未知类型/权限/值不可读/readonly/secure拒绝且零派发。

一次公共AXValue setter，UTF-8正文直接CFString，不剪贴板/脚本/进程桥。返回两次全文AXValue比对摘要（长度/hash/状态）；`observed_ax_replacement`不是DOM目标、composition或键盘授权。业务目标须独立AX/DOM readback；setter ack不等于目标。失败/timeout仅读原operation，不retry/type_text。

原双读明确不匹配且context仍可信时，追加只读settlement从原双读及context bracket完成后起预算≤100ms，仍受原operation/lease绝对deadline约束：无sleep、最多2次retained全文追加读，原first/second及initial状态不覆盖；settlement仅提供独立AX观察状态，不升DOM/键盘goal、不重setter，未知/取消/上下文变更原样保留。

候选尚未Cargo、部署、Native验收。原真BGChrome ASCII输入/emoji key229未插入证据保留；即使本路成功也不把旧keyboard Unicode或generic modern hover改判通过。生产需要新Agent与MCPschema，fresh loaded typed工具或native `rwi.call("computer.set_text", {..,lease})`，不要求用户重启Host。

当前复用的保守eligibility还要求已知AXFocused及AXFocused-writable。即使AXValue可写但focus属性不支持，也会拒绝；这不是focus/键盘授权，不执行focus setter。替换为destructive操作，两种MCP入口均明确标注。
