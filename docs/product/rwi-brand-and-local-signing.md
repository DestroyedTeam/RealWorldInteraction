# RWI 标识与本机签名

## 标识

使用内置 image_gen 生成：银白指针触及青色接触点，立体空间轨道，深墨背景，无文字/对号。

Prompt: Use case: logo-brand. A square app icon for Real World Interaction, a crisp ivory mouse pointer meeting an open spatial orbit, tiny luminous contact point, restrained industrial design, deep ink background, teal and silver accents, clear small-size silhouette, no text/checkmarks/robot/mockup.

主图 `integrations/codex/plugin/real-world-interaction/assets/logo.png`；256px 插件图标 `assets/icon.png`；macOS 多分辨率资源 `integrations/macos/assets/RWI.icns`。通过 sips/iconutil 做尺寸与格式转换，不重绘生成图。

## 本机签名

证书助理：RWI Local Development / Self Signed Root / Code Signing；私钥保存在登录钥匙串，不导出到仓库。创建前需要用户确认。采用长有效期并长期复用同一身份，不每次重新生成证书。

`tools/sign_rwi_macos_app.sh /absolute/RWI\ Agent.app CERTIFICATE_SHA1` 将统一图标加入包、设置 CFBundleIconFile，再签名并验证。必须显式指定40位证书SHA1，不允许自动退回 ad-hoc；不会更改 TCC 或重启。更换签名身份后首次仍需授权，并以两次不同构建保留旧 DR 和实际权限作为验收。

对外分发与本机签名分离：不分发本机私钥，不将自签名包描述为 Developer ID/notarized。未公证/未开发者签名包在目标系统的安装与启动体验需另外测试，不能承诺与公证版一致；本次没有创建发布版或发布tag。

参考：[Apple Code Signing Tasks](https://developer.apple.com/library/archive/documentation/Security/Conceptual/CodeSigningGuide/Procedures/Procedures.html)。

## 实际完成状态（2026-09-19）

- 用户确认后，通过系统 Certificate Assistant 创建 `RWI Local Development`，RSA2048、SHA256、仅Digital Signature + Code Signing，有效期至2036-09-16；个人邮箱未纳入证书。私钥直接保存在login钥匙串，无导出。
- 证书SHA1：`24B346A95EC8F450F7D8433A2E1876568DBCB2F8`。代码默认DR为identifier + certificate leaf，而非cdhash。
- 两次签名之间修改CFBundleVersion，由1到2；后者通过前者DR的 codesign --verify --strict -R 验证。此项验证代码身份稳定，不等于TCC更新保留实测。
- 已把带统一图标的证书签名应用部署到原安装路径并启动，备份在work/rwi-brand/previous-agent-bundle。应用CFBundleIconFile=RWI.icns，安装图标与仓库资源字节一致。
- Plugin已验证并重装为0.2.0+codex.20260919112651，安装缓存logo与仓库资产字节一致；图标界面缓存刷新尚未目视验证。
- 新Agent PID16839实测四项TCC权限false。临时签名到证书签名的首次迁移尚需授权，后续跨更新保留权限仍待现场验证，不作已完成承诺。
