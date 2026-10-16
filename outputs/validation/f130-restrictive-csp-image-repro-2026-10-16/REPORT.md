# F130：真实浏览器同图/CSP对照

F129实际进入decode_png，记录rpc_0（尚不能据此称RPC故障）。在隔离后台页面按[MCP Apps稳定规范](https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/2026-01-26/apps.mdx)的限制性img-src self data策略，对同一张未改动的专用窗口PNG：

- Blob：EncodingError/code0，img-src明确阻止blob。
- PNG data：正常解码1320×1104。
- SHA256：4d5ab96d30ea9ec48d9e312ca1c5ec1dddf605620072529e62581495badbdd2b。
- 没有新截屏、键鼠或MCP输入；只启动一次性Rust测试HTTP服务，已退出0。

这是隔离浏览器的因果对照，不是读取了Codex实际CSP响应头。F129负例保留。候选修正用PNG data替代Blob，不添加网络域、不增加权限、不做错误后的截图/输入回退。

![同图对照](01-real-browser-csp-comparison.jpg)
