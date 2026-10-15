//! Browser-content references never authorize native windows, input or OS dialogs.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionArgs {
    Create {
        device_id: String,
        request_id: String,
        #[serde(default = "blank")]
        url: String,
    },
    Close {
        device_id: String,
        request_id: String,
        session_ref: String,
    },
    Query {
        device_id: String,
        /// The original durable lifecycle request, not a new request/monitor ID.
        request_id: String,
    },
}
fn blank() -> String {
    "about:blank".into()
}
/// Read-only owned page content. Never accepts a native frame, coordinate or script.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SnapshotArgs {
    pub device_id: String,
    pub session_ref: String,
    #[serde(default = "snapshot_nodes")]
    pub max_nodes: u16,
}
fn snapshot_nodes() -> u16 {
    128
}
/// One explicitly authorized page action, never native coordinates or a script.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Hover,
    Click,
}
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActArgs {
    pub device_id: String,
    pub session_ref: String,
    pub request_id: String,
    pub observation_ref: String,
    pub element_ref: String,
    pub action: Action,
}
impl ActArgs {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !super::valid_id(&self.device_id)
            || !super::valid_id(&self.session_ref)
            || !self.session_ref.starts_with("browser:")
            || !super::valid_id(&self.request_id)
            || !super::valid_id(&self.observation_ref)
            || !self.observation_ref.starts_with("browser-observation:")
            || !self.element_ref.strip_prefix('e').is_some_and(|s| {
                !s.is_empty()
                    && s.bytes().all(|b| b.is_ascii_digit())
                    && s.parse::<u16>().is_ok_and(|n| (1..=512).contains(&n))
            })
        {
            return Err("browser_act_arguments_invalid");
        }
        Ok(())
    }
}
impl SnapshotArgs {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !super::valid_id(&self.device_id)
            || !super::valid_id(&self.session_ref)
            || !self.session_ref.starts_with("browser:")
            || !(1..=512).contains(&self.max_nodes)
        {
            return Err("browser_snapshot_arguments_invalid");
        }
        Ok(())
    }
}
impl SessionArgs {
    pub fn device_id(&self) -> &str {
        match self {
            Self::Create { device_id, .. }
            | Self::Close { device_id, .. }
            | Self::Query { device_id, .. } => device_id,
        }
    }
    pub fn request_id(&self) -> &str {
        match self {
            Self::Create { request_id, .. }
            | Self::Close { request_id, .. }
            | Self::Query { request_id, .. } => request_id,
        }
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if !super::valid_id(self.device_id()) || !super::valid_id(self.request_id()) {
            return Err("browser_device_or_request_id_invalid");
        }
        match self {
            Self::Create { url, .. }
                if url.is_empty()
                    || url.len() > 2048
                    || !url.is_ascii()
                    || url.bytes().any(|b| b.is_ascii_control()) =>
            {
                Err("browser_url_invalid")
            }
            Self::Close { session_ref, .. }
                if !super::valid_id(session_ref) || !session_ref.starts_with("browser:") =>
            {
                Err("browser_session_reference_invalid")
            }
            _ => Ok(()),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn f16_browser_content_input_typed_refs_never_accept_native_or_arbitrary_coordinates() {
        let valid = json!({"device_id":"mac","session_ref":"browser:one","request_id":"once","observation_ref":"browser-observation:one","element_ref":"e1","action":"hover"});
        let a: ActArgs = serde_json::from_value(valid.clone()).unwrap();
        assert!(a.validate().is_ok());
        assert!(super::super::catalog::validate("browser.act", &valid).is_ok());
        assert!(!super::super::catalog::executable("browser.act"));
        for key in [
            "target",
            "frame",
            "lease",
            "backend_node_id",
            "x",
            "y",
            "javascript",
            "fallback",
            "profile",
            "cdp_url",
        ] {
            let mut v = valid.clone();
            v[key] = json!("forged");
            assert!(serde_json::from_value::<ActArgs>(v).is_err(), "{key}");
        }
        for (k, v) in [
            ("element_ref", json!("e0")),
            ("element_ref", json!("e513")),
            ("element_ref", json!("7")),
            ("session_ref", json!("native:one")),
            ("observation_ref", json!("native:frame")),
        ] {
            let mut bad = valid.clone();
            bad[k] = v;
            assert!(
                serde_json::from_value::<ActArgs>(bad)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
        let mut v = valid;
        v["action"] = json!("drag");
        assert!(serde_json::from_value::<ActArgs>(v).is_err());
        let defs = super::super::catalog::definitions();
        let d = defs.iter().find(|d| d.name == "browser.act").unwrap();
        assert!(d.writes);
        assert_eq!(d.input_schema["properties"].as_object().unwrap().len(), 6);
    }
    #[test]
    fn f15_browser_content_snapshot_typed_read_rejects_native_authority_and_unbounded_size() {
        let valid = json!({"device_id":"mac","session_ref":"browser:one"});
        let a: SnapshotArgs = serde_json::from_value(valid.clone()).unwrap();
        assert_eq!(a.max_nodes, 128);
        assert!(a.validate().is_ok());
        for key in [
            "target",
            "frame",
            "lease",
            "javascript",
            "cdp_url",
            "profile",
            "request_id",
        ] {
            let mut v = valid.clone();
            v[key] = json!("forged");
            assert!(serde_json::from_value::<SnapshotArgs>(v).is_err());
        }
        for max in [0, 513] {
            let mut v = valid.clone();
            v["max_nodes"] = json!(max);
            assert!(
                serde_json::from_value::<SnapshotArgs>(v)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
        let mut v = valid.clone();
        v["session_ref"] = json!("native:one");
        assert!(
            serde_json::from_value::<SnapshotArgs>(v)
                .unwrap()
                .validate()
                .is_err()
        );
        assert!(!super::super::catalog::executable("browser.snapshot"));
        assert!(super::super::catalog::validate("browser.snapshot", &valid).is_ok());
        let defs = super::super::catalog::definitions();
        assert!(
            !defs
                .iter()
                .find(|d| d.name == "browser.snapshot")
                .unwrap()
                .writes
        );
    }
    #[test]
    fn f14_browser_typed_family_rejects_native_authority_and_extra_fields() {
        let a = json!({"action":"create","device_id":"mac","request_id":"one"});
        let a: SessionArgs = serde_json::from_value(a).unwrap();
        assert!(a.validate().is_ok());
        let SessionArgs::Create { url, .. } = a else {
            panic!()
        };
        assert_eq!(url, "about:blank");
        for key in [
            "target",
            "frame",
            "lease",
            "cdp_url",
            "profile",
            "javascript",
        ] {
            let mut a = json!({"action":"create","device_id":"mac","request_id":"one"});
            a[key] = json!("forged");
            assert!(serde_json::from_value::<SessionArgs>(a).is_err());
        }
        assert!(
            serde_json::from_value::<SessionArgs>(
                json!({"action":"hover","device_id":"mac","request_id":"one"})
            )
            .is_err()
        );
        let bad: SessionArgs = serde_json::from_value(json!({"action":"close","device_id":"mac","request_id":"one","session_ref":"native:one"})).unwrap();
        assert!(bad.validate().is_err());
        assert!(!super::super::catalog::executable("browser.session"));
    }
}
