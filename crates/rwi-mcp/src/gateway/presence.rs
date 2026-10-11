//! Loopback-only Host-panel visibility publisher. No screen, input or credentials
//! are served over HTTP. The authenticated device context remains in Gateway.
use super::*;
use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

const MAX_VIEWS: usize = 16;
const MAX_HTTP: usize = 4096;
#[derive(Default)]
pub(super) struct Views {
    address: tokio::sync::Mutex<Option<SocketAddr>>,
    entries: Mutex<BTreeMap<String, View>>,
}
struct View {
    target: String,
    binding: String,
    page: Option<String>,
    ui_sequence: u64,
    device_sequence: u64,
    expires: Instant,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Event {
    page: String,
    sequence: u64,
    visible: bool,
}
struct HttpRequest {
    method: String,
    path: String,
    host: String,
    origin: Option<String>,
    body: Vec<u8>,
}
fn token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
fn err(code: &str, message: &str) -> DeviceResult {
    DeviceResult::error(code, message)
}

impl Views {
    #[cfg(test)]
    pub(super) fn test_app_page(&self, target: &str) -> (String, String, rwi_core::LiveViewUpdate) {
        self.register(target, 1000).unwrap();
        self.app_page(target).unwrap()
    }
    fn register(&self, target: &str, duration_ms: u64) -> Result<(String, String), DeviceResult> {
        let mut views = self
            .entries
            .lock()
            .map_err(|_| err("live_view_unavailable", "View registry unavailable."))?;
        views.retain(|_, v| v.expires > Instant::now());
        if let Some((key, v)) = views.iter_mut().find(|(_, v)| v.target == target) {
            v.expires = Instant::now() + Duration::from_millis(duration_ms);
            return Ok((key.clone(), v.binding.clone()));
        }
        if views.len() >= MAX_VIEWS {
            return Err(err(
                "live_view_capacity",
                "Close an existing preview before starting another.",
            ));
        }
        let key = token();
        let binding = token();
        views.insert(
            key.clone(),
            View {
                target: target.into(),
                binding: binding.clone(),
                page: None,
                ui_sequence: 0,
                device_sequence: 0,
                expires: Instant::now() + Duration::from_millis(duration_ms),
            },
        );
        Ok((key, binding))
    }
    pub(super) fn remove_target(&self, target: &str) {
        if let Ok(mut views) = self.entries.lock() {
            views.retain(|_, v| v.target != target);
        }
    }
    fn refresh_lifetime(&self, target: &str, remaining_ms: u64) {
        if let Ok(mut views) = self.entries.lock() {
            for view in views.values_mut().filter(|view| view.target == target) {
                view.expires = Instant::now() + Duration::from_millis(remaining_ms.min(600_000));
            }
        }
    }
    fn new_page(&self, key: &str) -> Option<(String, String, rwi_core::LiveViewUpdate)> {
        let mut views = self.entries.lock().ok()?;
        let v = views.get_mut(key)?;
        if v.expires <= Instant::now() {
            return None;
        }
        let page = token();
        v.page = Some(page.clone());
        v.ui_sequence = 0;
        v.device_sequence = v.device_sequence.checked_add(1)?;
        Some((
            page,
            v.target.clone(),
            rwi_core::LiveViewUpdate {
                view_id: v.binding.clone(),
                sequence: v.device_sequence,
                visible: false,
            },
        ))
    }
    pub(super) fn app_page(
        &self,
        target: &str,
    ) -> Option<(String, String, rwi_core::LiveViewUpdate)> {
        let key = self
            .entries
            .lock()
            .ok()?
            .iter()
            .find(|(_, v)| v.target == target)?
            .0
            .clone();
        let (page, _, update) = self.new_page(&key)?;
        Some((key, page, update))
    }
    pub(super) fn app_event(
        &self,
        key: &str,
        page: String,
        sequence: u64,
        visible: bool,
    ) -> Result<(String, rwi_core::LiveViewUpdate), &'static str> {
        self.event(
            key,
            Event {
                page,
                sequence,
                visible,
            },
        )
    }
    fn event(
        &self,
        key: &str,
        event: Event,
    ) -> Result<(String, rwi_core::LiveViewUpdate), &'static str> {
        let mut views = self.entries.lock().map_err(|_| "view_unavailable")?;
        let v = views.get_mut(key).ok_or("view_ended")?;
        if v.expires <= Instant::now() || v.page.as_deref() != Some(&event.page) {
            return Err("view_ended");
        }
        if event.sequence == 0 || event.sequence <= v.ui_sequence {
            return Err("stale_event");
        }
        v.ui_sequence = event.sequence;
        v.device_sequence = v.device_sequence.checked_add(1).ok_or("view_ended")?;
        Ok((
            v.target.clone(),
            rwi_core::LiveViewUpdate {
                view_id: v.binding.clone(),
                sequence: v.device_sequence,
                visible: event.visible,
            },
        ))
    }
}
impl Gateway {
    pub(super) async fn host_live(
        &self,
        mut args: Value,
        context: Option<RequestContext<RoleServer>>,
    ) -> DeviceResult {
        if args.get("view").is_some_and(|v| !v.is_null())
            || args
                .pointer("/options/host_view_id")
                .is_some_and(|v| !v.is_null())
        {
            return err(
                "host_view_reserved",
                "Host visibility is published only by the task-owned view, not tool arguments.",
            );
        }
        let a = match serde_json::from_value::<device::api::LiveArgs>(args.clone()) {
            Ok(a) => a,
            Err(e) => return err("invalid_arguments", &e.to_string()),
        };
        if a.action != rwi_core::LiveAction::Start {
            if a.action == rwi_core::LiveAction::Stop {
                self.presence.remove_target(&a.target);
            }
            let result = self.invoke_device("computer.live", args, context).await;
            if !result.is_error
                && a.action == rwi_core::LiveAction::Resume
                && let Some(ms) = result.data["expires_in_ms"].as_u64()
            {
                self.presence.refresh_lifetime(&a.target, ms);
            }
            return result;
        }
        let Some(options) = a.options else {
            return err(
                "invalid_live_options",
                "Start needs an explicit viewer display.",
            );
        };
        if let Err(e) = options.validate() {
            return err(&e.code, &e.next_step);
        }
        let address = match self.presence_server().await {
            Ok(a) => a,
            Err(e) => return err("live_view_server_unavailable", &e.to_string()),
        };
        let (key, binding) = match self.presence.register(&a.target, options.duration_ms) {
            Ok(v) => v,
            Err(e) => return e,
        };
        args["options"]["host_view_id"] = json!(binding);
        let mut result = self.invoke_device("computer.live", args, context).await;
        if result.is_error {
            self.presence.remove_target(&a.target);
            return result;
        }
        if result.data["host_view_binding"] != binding {
            self.presence.remove_target(&a.target);
            return err(
                "live_view_binding_unconfirmed",
                "An older or already-running unbound preview cannot be claimed as Host-bound. Stop it explicitly before starting again.",
            );
        }
        if let Some(ms) = result.data["expires_in_ms"].as_u64() {
            self.presence.refresh_lifetime(&a.target, ms);
        }
        result.data["viewer"] = json!({
            "url":format!("http://{address}/{key}/"),
            "open_with":"open_in_codex browser in the current task",
            "binding":"visibility of this task-owned browser panel; closing or covering that panel also hides the preview",
            "unavailable_signal":"hide and clear after the short device-side presence lease; never infer active task from an MCP connection"
        });
        result
    }
    async fn presence_server(&self) -> io::Result<SocketAddr> {
        let mut address = self.presence.address.lock().await;
        if let Some(address) = *address {
            return Ok(address);
        }
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let bound = listener.local_addr()?;
        let gateway = self.clone();
        let mut shutdown = self.shutdown.subscribe();
        tokio::spawn(async move {
            let limit = Arc::new(tokio::sync::Semaphore::new(8));
            loop {
                tokio::select! {biased;
                    _=shutdown.changed()=>break,
                    accepted=listener.accept()=>{
                        let Ok((stream,_))=accepted else{break};
                        let Ok(permit)=limit.clone().try_acquire_owned() else{drop(stream);continue};
                        let gateway=gateway.clone();
                        tokio::spawn(async move {let _permit=permit;let _=tokio::time::timeout(Duration::from_secs(2),gateway.view_http(stream,bound)).await;});
                    }
                }
            }
            if let Ok(mut entries) = gateway.presence.entries.lock() {
                entries.clear();
            }
        });
        *address = Some(bound);
        Ok(bound)
    }
    pub(super) async fn publish_view(
        &self,
        target: &str,
        update: rwi_core::LiveViewUpdate,
    ) -> DeviceResult {
        self.invoke_device(
            "computer.live",
            json!({"target":target,"action":"status","view":update}),
            None,
        )
        .await
    }
    async fn view_http(&self, mut stream: TcpStream, address: SocketAddr) -> io::Result<()> {
        let req = read_http(&mut stream).await?;
        let origin = format!("http://{address}");
        if req.host != address.to_string() {
            return reply(&mut stream, 403, "text/plain", "", "Rejected Host").await;
        }
        let Some((key, suffix)) = req.path.strip_prefix('/').and_then(|p| p.split_once('/')) else {
            return reply(&mut stream, 404, "text/plain", "", "Not found").await;
        };
        if key.len() != 32 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
            return reply(&mut stream, 404, "text/plain", "", "Not found").await;
        }
        if req.method == "GET" && suffix.is_empty() {
            let Some((page, target, hidden)) = self.presence.new_page(key) else {
                return reply(&mut stream, 410, "text/plain", "", "Preview has ended").await;
            };
            let result = self.publish_view(&target, hidden).await;
            if result.is_error {
                return reply(
                    &mut stream,
                    503,
                    "text/plain",
                    "",
                    "Device unavailable; preview remains hidden",
                )
                .await;
            }
            let nonce = token();
            let html = include_str!("presence.html")
                .replace("__NONCE__", &nonce)
                .replace("__PAGE__", &page);
            return reply(&mut stream, 200, "text/html; charset=utf-8", &nonce, &html).await;
        }
        if req.method == "POST" && suffix == "presence" && req.origin.as_deref() == Some(&origin) {
            let update = serde_json::from_slice::<Event>(&req.body)
                .map_err(|_| "invalid_event")
                .and_then(|event| self.presence.event(key, event));
            let (target, update) = match update {
                Ok(update) => update,
                Err(reason) => {
                    return reply(
                        &mut stream,
                        if reason == "stale_event" { 409 } else { 410 },
                        "application/json",
                        "",
                        &json!({"accepted":false,"reason":reason}).to_string(),
                    )
                    .await;
                }
            };
            let result = self.publish_view(&target, update).await;
            let body = if result.is_error {
                json!({"accepted":false,"error":result.data["code"]})
            } else {
                result.data
            };
            return reply(
                &mut stream,
                if result.is_error { 503 } else { 200 },
                "application/json",
                "",
                &body.to_string(),
            )
            .await;
        }
        reply(&mut stream, 403, "text/plain", "", "Rejected request").await
    }
}
async fn read_http(stream: &mut TcpStream) -> io::Result<HttpRequest> {
    let mut bytes = Vec::new();
    let mut buf = [0u8; 1024];
    let end = loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            return Err(io::Error::other("incomplete HTTP"));
        }
        bytes.extend_from_slice(&buf[..n]);
        if bytes.len() > MAX_HTTP {
            return Err(io::Error::other("HTTP budget"));
        }
        if let Some(i) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let header = std::str::from_utf8(&bytes[..end]).map_err(io::Error::other)?;
    let mut lines = header.lines();
    let first = lines
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>();
    if first.len() != 3 || first[2] != "HTTP/1.1" {
        return Err(io::Error::other("HTTP request line"));
    }
    let (method, path) = (first[0].to_owned(), first[1].to_owned());
    let mut fields = BTreeMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (k, v) = line
            .split_once(':')
            .ok_or_else(|| io::Error::other("HTTP field"))?;
        if fields
            .insert(k.to_ascii_lowercase(), v.trim().to_owned())
            .is_some()
        {
            return Err(io::Error::other("duplicate HTTP field"));
        }
    }
    if fields.contains_key("transfer-encoding") {
        return Err(io::Error::other("chunked request unsupported"));
    }
    let length = fields
        .get("content-length")
        .map(|s| s.parse::<usize>())
        .transpose()
        .map_err(io::Error::other)?
        .unwrap_or(0);
    if length > 1024 || end + length > MAX_HTTP {
        return Err(io::Error::other("HTTP body budget"));
    }
    while bytes.len() < end + length {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            return Err(io::Error::other("incomplete body"));
        }
        bytes.extend_from_slice(&buf[..n]);
        if bytes.len() > MAX_HTTP {
            return Err(io::Error::other("HTTP budget"));
        }
    }
    Ok(HttpRequest {
        method,
        path,
        host: fields.remove("host").unwrap_or_default(),
        origin: fields.remove("origin"),
        body: bytes[end..end + length].to_vec(),
    })
}
async fn reply(
    stream: &mut TcpStream,
    code: u16,
    mime: &str,
    nonce: &str,
    body: &str,
) -> io::Result<()> {
    let csp = format!(
        "default-src 'none'; script-src 'nonce-{nonce}'; style-src 'nonce-{nonce}'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'"
    );
    let response = format!(
        "HTTP/1.1 {code} RWI\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: {csp}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replaced_page_and_out_of_order_events_do_not_renew_presence() {
        let views = Views::default();
        let (key, binding) = views.register("device:target", 1000).unwrap();
        let (page, _, first) = views.new_page(&key).unwrap();
        assert!(!first.visible);
        assert_eq!(first.view_id, binding);
        let (_, visible) = views
            .event(
                &key,
                Event {
                    page: page.clone(),
                    sequence: 2,
                    visible: true,
                },
            )
            .unwrap();
        assert!(visible.visible);
        assert!(
            views
                .event(
                    &key,
                    Event {
                        page: page.clone(),
                        sequence: 1,
                        visible: false
                    }
                )
                .is_err()
        );
        let (new_page, _, hidden) = views.new_page(&key).unwrap();
        assert!(hidden.sequence > visible.sequence);
        assert!(
            views
                .event(
                    &key,
                    Event {
                        page,
                        sequence: 3,
                        visible: true
                    }
                )
                .is_err()
        );
        assert!(
            views
                .event(
                    &key,
                    Event {
                        page: new_page,
                        sequence: 1,
                        visible: true
                    }
                )
                .unwrap()
                .1
                .sequence
                > hidden.sequence
        );
        views.remove_target("device:target");
        assert!(views.new_page(&key).is_none());
    }
}
