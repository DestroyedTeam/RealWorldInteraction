# macOS 显式接收组件：接入与能力边界

## 适用范围

这是**目标应用主动集成**的 Rust/AppKit SDK，不是向已安装应用注入代码的工具。
M06 合同 revision 6 已由用户明确批准这一交付范围；原来的普通 AppKit
同 PID 兄弟窗口获焦失败仍保留。没有接入组件的应用不能据此声明支持后台保焦点。

有两种显式接入形式：

1. 已有自有 `NSApplication::sendEvent:` 接入点时，调用 `Registration::dispatch`。
2. 不改 `NSApplication` 子类时，安装应用自身的公开 AppKit local monitor。
   本次交付优先采用第二种；不要同时启用两种派发方式。

组件只处理已注册自有 NSView/NSWindow 的普通左键 down/drag/up 原事件，
不重新构造事件、不激活窗口、不先抢焦点再恢复、不静默切换输入路线。
它不声明 hover、滚轮、右键、键盘、IME 或任意第三方控件也有同样能力。

## 接入示例

在应用主线程、窗口已显示且 view 已实际附着后，用当前获授权 Agent 的真实 PID 注册：

```rust,ignore
use std::rc::Rc;
use objc2::runtime::AnyObject;
use rwi_macos::receiver_component::{LocalMonitor, Registration};

fn install(
    own_view: &AnyObject,
    own_window: &AnyObject,
    actual_agent_pid: u32,
) -> Result<LocalMonitor, String> {
    let registration = Rc::new(Registration::register(
        own_view,
        own_window,
        actual_agent_pid,
    )?);
    registration.install_local_monitor()
}
```

应用必须持有返回的 `LocalMonitor`。不要把它作为临时值立即丢弃。
它不可跨线程移动；安装、停止及窗口释放均在主线程执行。
关闭窗口或销毁接收视图前调用 `monitor.stop()?`：第一次返回 `true`，
重复停止返回 `false`。`Drop` 是额外兜底，不替代清晰的应用生命周期管理。
`is_installed()` 只表示监视器 token 仍被持有，不表示某项业务效果已经验证。

local monitor 对不属于注册窗口/来源的事件返回原指针，维持应用正常派发；
对已接收事件返回 nil，避免 superclass 再派发一次。
不得将 `dispatch` 返回 true 的事件再传给窗口、重试或转换为激活操作。
不可把两个 receiver 注册到重叠区域后推断自动仲裁。

### 来源与安全

来源 PID、process birth、窗口对象及窗口号是派发过滤器，**不是密码学身份认证**。
`Stats::source_authenticated` 明确为 false；不要用该组件代替 Agent 的设备配对、
授权、租约及持久去重。SDK 不自动寻找可信 Agent，也不保证 PID 字段不可伪造。
安装前应由应用自己的可信配置传入目标身份；Agent 更新导致 PID 改变后，
显式停止旧注册，再按新的已验证身份重新注册，不复用旧请求。

## 本次实机结果

测试环境为当前 macOS 27.2、自有普通 AppKit 双窗口、已分配虚拟屏。
通过当前真实 Host 加载的 RWI MCP 发出一次点击及一次 1 秒拖拽：

| 条件 | 独立观察 |
| --- | --- |
| local monitor 已安装 | click down/up/click = 1/1/1；drag 新增 down/move/up = 1/16/1，蓝线完成 |
| 同 PID 兄弟窗口原先为 key | 完整 OS 前台边界元组一致，自有 key 通知未变化；manual dispatch 调用数为 0 |
| 显式停止 | stop 依次 true/false，installed=false |
| 停止后的另一个新点击 | SDK 计数不增长，普通 AppKit 令主窗获焦，原限制复现 |
| 清理 | 五个原操作终态、资源释放、无重放；测试 App 正常退出，两窗口绑定退休 |

这不是连续全局 OS 证明或物理人工切窗证明，也不证明所有应用兼容。
源事件的过滤仍依赖当前 macOS 行为，不能当作跨 OS 私有 ABI 稳定承诺。

## 验证命令

默认离线测试不会安装监视器或操作用户窗口：

```sh
rtk cargo test --offline --locked -p rwi-macos --lib f404_monitor -- --nocapture
rtk cargo test --offline --locked -p rwi-macos --example receiver_fixture f404_fixture -- --nocapture
rtk cargo fmt --all -- --check
rtk cargo test --workspace --locked --offline
rtk cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
```

专用 fixture 的 `RWI_EXPERIMENTAL_RECEIVER_LOCAL_MONITOR=1` 默认关闭，且必须显式
提供合法来源 PID、regular app 和环境采样配置。它是测试入口，不是给现有应用注入的命令。
仅构建 SDK/fixture 不会重启生产 Agent、替换 MCP、修改证书或重新请求 TCC。

公开 API 的设计依据：[Apple Event Monitoring](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/MonitoringEvents/MonitoringEvents.html)。
实测证据及原始截图由本机验收报告保留，原始截图、运行状态和二进制不提交 Git。
