# R69：显式菜单动作、回归与稳定签名部署

## 最小修改与守卫

macOS desktop.rs 的 native_semantic_actions 增加AXShowMenu、AXCancel、AXPick。native dispatcher本来已有精确object/window/provider/state重验，本轮不改其执行体、默认click或pointer/focus路径，不自动fallback/retry。AXPick在本机SDK列Obsolete，仅限真实控件advertise与调用方明确选择，不默认替代AXPress。

核心fixture参数化动作，新增能力/element动作门禁、同request去重与错payload拒绝；原禁用/状态变更负例保留。Mac声明及匹配reference/click-only测试扩展，不允许pointer reference升级。

## 真实门禁与回滚

完整11门exit0：workspace776 passed/0 failed/11 ignored/42 suites、receiver72、LiveUI7；fmt、locked/offline tests、all-target/all-feature strict Clippy、release MCP/Agent与架构正负例。3 focused命令各1test通过，不重复加到workspace。

初source probe因regex错误exit1保留，不是有效baseline。纠正后真正原pristine配置false/1→modified true/0→独立copy窄ROLLBACK恢复三原始hash/probe1→reapply0。原97baseline不变，生产未回滚。

## 部署及当前会话

沿用证书24B346A95EC8F450F7D8433A2E1876568DBCB2F8及同DR。原inode排他lease/旧instance durable全终态门通过后原子交换.app；新Agent89552/cabc313d-ff5c-4d7f-b2cf-f24a65d527c4，signed SHA05e6e1ad668e7c78ea0682d5013fec144c61a8f95a941911060fae12b0ffda10。四权限true、CA/profile/config/MCP hash未变，无人工授权/重启Codex。旧APP和独立baseline.app保留。

本任务已加载MCP真正读到新instance和3动作，不靠stdio catalog替代GUI。R70实测3动作的语义效果与有界等待恢复；菜单像素/foreign-provider仍未接受。

依据：[Apple AXPick](https://developer.apple.com/documentation/applicationservices/kaxpickaction)、[AXCancel](https://developer.apple.com/documentation/applicationservices/kaxcancelaction)、[AXShowMenu](https://developer.apple.com/documentation/applicationservices/kaxshowmenuaction)。网页Markdown链接404/JS壳与本机SDK完整声明都已保留，未伪造网页正文。全目标未完成。
