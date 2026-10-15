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
