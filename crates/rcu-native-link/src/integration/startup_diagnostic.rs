//! Unavailable-session MCP diagnostics, not an alternative native backend.
//! Recorded tool schemas remain discoverable; every business call fails closed.
use crate::{invalid, mcp, sha};
use rmcp::{
    ErrorData, RoleServer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResult, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
};
use serde_json::{Value, json};
use std::{io, sync::Arc};

#[derive(Clone)]
pub struct Unavailable {
    task: String,
    reason: String,
    tools: Arc<Vec<Tool>>,
}
impl Unavailable {
    pub fn new(task: &str, reason: &str) -> io::Result<Self> {
        let source: Value =
            serde_json::from_str(include_str!("../../fixtures/golden-schema.json"))?;
        let mut tools = source["tools"]
            .as_array()
            .ok_or_else(|| invalid("recorded_schema_missing"))?
            .clone();
        tools.sort_by_key(|t| t["name"].as_str().unwrap_or_default().to_owned());
        if tools.len() != 19 || sha(&serde_json::to_vec(&tools)?) != mcp::schema_sha() {
            return Err(invalid("recorded_schema_digest_mismatch"));
        }
        Ok(Self {
            task: task.into(),
            reason: reason.chars().take(256).collect(),
            tools: Arc::new(serde_json::from_value(Value::Array(tools))?),
        })
    }
    fn blocked(&self, tool: &str) -> Result<CallToolResult, ErrorData> {
        if !self.tools.iter().any(|t| t.name.as_ref() == tool) {
            return Err(ErrorData::invalid_params("unknown computer tool", None));
        }
        Ok(CallToolResult::structured_error(json!({
            "code":"native_session_unavailable","task":self.task,"startup_error":self.reason,
            "mode":"diagnostic_only","tool_catalog_source":"recorded_digest_checked_worker_schema",
            "native_worker_verified":false,"native_target_enumerated":false,
            "action_authorized":false,"operation_dispatched":false,"goal_verified":false,
            "automatic_reconnect":false,"operation_replayed":false,
            "next_step":"Inspect controller status and prepare a fresh task for the requested scope. Keep MCP enabled. Do not reset HOLD, retry the lost session, or use a shell/legacy bridge to perform the action."
        })))
    }
}
impl ServerHandler for Unavailable {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("real-computer-use-native-link",env!("CARGO_PKG_VERSION")))
            .with_instructions("DIAGNOSTIC ONLY: native session startup failed. Recorded computer tool schemas are visible, but all calls fail closed and no native action is possible. Tool discovery is not native readiness. Read computer.targets for the failure; prepare a fresh scoped task, never replay HOLD or fall back to shell input.")
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: self.tools.as_ref().clone(),
            ..Default::default()
        })
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        self.blocked(&request.name)
    }
}
pub async fn serve(task: &str, reason: &str) -> io::Result<()> {
    use tokio::io::{AsyncWriteExt, BufReader};
    struct AbortFeed(tokio::task::AbortHandle);
    impl Drop for AbortFeed {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    // Preserve the relay's original line and total-request bounds before the
    // SDK decodes anything; unavailable mode is not a larger input surface.
    let (mut bounded_write, bounded_read) = tokio::io::duplex(crate::frame::INBOUND);
    let feed = tokio::spawn(async move {
        let mut input = BufReader::new(tokio::io::stdin());
        for _ in 0..crate::net::MAX_CLIENT_FRAMES {
            match crate::frame::read_frame(&mut input, crate::frame::INBOUND).await? {
                Some(raw) => bounded_write.write_all(&raw).await?,
                None => return Ok::<_, io::Error>(()),
            }
        }
        Err(invalid("diagnostic_request_budget_exhausted"))
    });
    let _abort = AbortFeed(feed.abort_handle());
    let service = Unavailable::new(task, reason)?
        .serve((bounded_read, tokio::io::stdout()))
        .await
        .map_err(io::Error::other)?;
    service.waiting().await.map_err(io::Error::other)?;
    feed.await.map_err(io::Error::other)??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recorded_catalog_is_explicitly_not_native_readiness() {
        let s = Unavailable::new("held-task", "attach_ack_missing").unwrap();
        assert_eq!(s.tools.len(), 19);
        assert!(
            s.get_info()
                .instructions
                .unwrap()
                .starts_with("DIAGNOSTIC ONLY")
        );
        for t in s.tools.iter() {
            let r = s.blocked(&t.name).unwrap();
            assert_eq!(r.is_error, Some(true));
            let d = r.structured_content.unwrap();
            for key in [
                "native_worker_verified",
                "native_target_enumerated",
                "action_authorized",
                "operation_dispatched",
                "goal_verified",
                "automatic_reconnect",
                "operation_replayed",
            ] {
                assert_eq!(d[key], false);
            }
        }
        assert!(s.blocked("controller.prepare").is_err());
    }
}
