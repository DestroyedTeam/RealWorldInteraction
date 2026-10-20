//! Bounded, opt-in debug-only shutdown phase observation. Never logs MCP payloads.
//! Release builds use the unwrapped SDK transport and have no environment gate.
pub fn transport<T, E, A>(
    transport: T,
) -> impl rmcp::transport::Transport<rmcp::RoleServer, Error = E> + 'static
where
    T: rmcp::transport::IntoTransport<rmcp::RoleServer, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    wrap(transport.into_transport())
}

#[cfg(debug_assertions)]
mod debug {
    use rmcp::{
        RoleServer,
        service::{RxJsonRpcMessage, TxJsonRpcMessage},
        transport::Transport,
    };
    use std::{
        sync::{
            OnceLock,
            atomic::{AtomicUsize, Ordering},
        },
        time::Instant,
    };
    static ENABLED: OnceLock<bool> = OnceLock::new();
    static STARTED: OnceLock<Instant> = OnceLock::new();
    static EVENTS: AtomicUsize = AtomicUsize::new(0);

    pub fn phase(name: &'static str) {
        if !*ENABLED
            .get_or_init(|| std::env::var_os("RCU_MCP_EOF_DIAGNOSTICS").is_some_and(|v| v == "1"))
        {
            return;
        }
        let started = STARTED.get_or_init(Instant::now);
        let sequence = EVENTS.fetch_add(1, Ordering::Relaxed);
        if sequence < 512 {
            use std::io::Write;
            let _ = writeln!(
                std::io::stderr(),
                "RCU_EOF_PHASE {{\"sequence\":{sequence},\"elapsed_us\":{},\"phase\":\"{name}\"}}",
                started.elapsed().as_micros()
            );
        }
    }

    pub struct Observed<T>(T);
    pub fn wrap<T: Transport<RoleServer>>(transport: T) -> Observed<T> {
        Observed(transport)
    }
    impl<T: Transport<RoleServer>> Transport<RoleServer> for Observed<T> {
        type Error = T::Error;

        fn send(
            &mut self,
            item: TxJsonRpcMessage<RoleServer>,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
            let send = self.0.send(item);
            async move {
                phase("transport_send_begin");
                let result = send.await;
                phase(if result.is_ok() {
                    "transport_send_ok"
                } else {
                    "transport_send_error"
                });
                result
            }
        }

        async fn receive(&mut self) -> Option<RxJsonRpcMessage<RoleServer>> {
            let result = self.0.receive().await;
            phase(if result.is_some() {
                "transport_receive_message"
            } else {
                "transport_receive_eof"
            });
            result
        }

        async fn close(&mut self) -> Result<(), Self::Error> {
            phase("transport_close_begin");
            let result = self.0.close().await;
            phase(if result.is_ok() {
                "transport_close_ok"
            } else {
                "transport_close_error"
            });
            result
        }
    }
}
#[cfg(debug_assertions)]
pub use debug::{phase, wrap};

#[cfg(not(debug_assertions))]
pub fn phase(_: &'static str) {}

#[cfg(not(debug_assertions))]
pub fn wrap<T>(transport: T) -> T {
    transport
}
