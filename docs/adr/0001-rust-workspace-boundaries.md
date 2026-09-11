# ADR 0001：单 workspace、明确边界、薄进程入口

日期：2026-09-11。状态：采用，渐进实施；不是全量架构完成声明。

## 查阅的一手项目

1. [rust-analyzer Architecture](https://rust-analyzer.github.io/book/contributing/architecture.html)：借鉴 workspace、明确 API 边界、各模块写明“不知道什么”的架构约束，以及独立工程任务入口。RCU 业务契约不应知道 Codex 插件格式，模型摘要不能成为原生执行依据。不照搬编译器分层、增量数据库或 crate 数量。
2. [RustDesk File Structure](https://github.com/rustdesk/rustdesk#file-structure)：借鉴捕获、平台输入、客户端连接和服务端能力的分离。不引入其 Flutter、剪贴板、穿透服务或实现代码；自身授权、不可重放和独立验证仍是核心约束。
3. [Tokio Framing](https://tokio.rs/tokio/tutorial/framing)：借鉴帧解析与连接 I/O 分离、部分帧/EOF 明确定义和缓冲边界。教程不是生产服务器模板；原有上限、TLS 固定身份、取消和未知结果 HOLD 不能被简化掉。

这些是架构依据，不是“成熟项目都长同一种目录树”。本次没有复制第三方代码或依赖，也没有替换许可证。

## 采用的工程骨架

```text
Cargo.toml / Cargo.lock           唯一产品 workspace
crates/
  rcu-core/                      当前业务契约、用例和安全状态；后续分离内部职责
  rcu-imaging/                   纯图像映射/编码边界
  rcu-windows/                   Windows 原生适配、会话/令牌与生命周期
  rcu-macos/                     macOS 原生适配
  rcu-link-trust/                私有配对、证书与身份校验
  rcu-native-link/
    src/integration/             AI host 插件与模型输出投影
    src/...                      后续按协议/传输/编排/部署迁移，不增空壳
  rcu-mcp/                       进程装配 + MCP 适配；入口逐步变薄
integrations/codex/              使用说明；Rust 生成插件，不提交本机绑定配置
architecture/                   依赖规则、大文件债务上限
tools/                          工程检查，不是生产桥接
docs/adr/                       重要架构决策与取舍
docs/migrations/                来源、SHA 与历史迁移记录
docs/validation/                小型脱敏验收索引
work/                           忽略的缓存/临时状态；不是开发源码根
outputs/                        忽略的大型产物；显式归档，不默认提交
```

## 依赖与实现约束

- 领域/安全核心不能反向依赖 MCP、AI host、UIA 或 TLS 实现。
- 平台接口返回可核验事实；模型输出投影不能修改 `Scene`、授权或执行日志。
- 二进制只装配依赖和进程生命周期；工具参数、路由、回包和平台启动分属明确模块。
- 重构提交尽量只移动/封装职责；行为修复另开提交，原始失败测试保留。
- 当前 `rcu-native-link → rcu-windows`、核心存储/流程耦合仍是债务。先建立边界，不以一次大搬家冒充解耦。
- 不为模仿项目增加数据库、DI 框架、消息总线或大量小 crate。工程任务后续可收敛为 Rust `xtask`；本次先保留明确标识的工程检查脚本，不阻塞原生入口交付。

## 验收

根目录独立编译与测试；协作者不必寻找 `work/*/source`；能按模块职责定位代码；依赖检查拦截新增反向依赖；每次行为更改可回滚；原生平台验收与离线/Host 接入证据分别报告。
