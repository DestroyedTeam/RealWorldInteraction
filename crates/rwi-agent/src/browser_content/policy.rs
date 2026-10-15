//! Local operator policy, not model-controlled config or inherited native grants.
use crate::config::{Config, PlatformPolicy, Principal};
use rwi_link_trust::PeerPin;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, io, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Policy {
    pub version: u16,
    pub user_uid: u32,
    pub chrome_executable: PathBuf,
    pub chrome_sha256: String,
    /// Exact initial URL allowlist only. Not a browser network/firewall policy.
    pub initial_urls: Vec<String>,
    pub principals: Vec<Grant>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Grant {
    pub certificate: PeerPin,
    pub observe: bool,
    pub create: bool,
    pub close: bool,
    /// Lifecycle observation never silently becomes permission to read page content.
    #[serde(default)]
    pub snapshot: bool,
}
impl Policy {
    pub fn read(config: &Config) -> io::Result<Self> {
        if !cfg!(target_os = "macos") {
            return Err(io::Error::other("browser_platform_unsupported"));
        }
        let raw = rwi_link_trust::read_private(
            &config.state_root.join("browser-content-policy.json"),
            65536,
        )?;
        let p: Self = serde_json::from_slice(&raw)?;
        p.validate(config)?;
        Ok(p)
    }
    pub fn validate(&self, config: &Config) -> io::Result<()> {
        let PlatformPolicy::MacOs { user_uid, .. } = &config.platform else {
            return Err(io::Error::other("browser_platform_unsupported"));
        };
        if self.version != 1
            || self.user_uid != *user_uid
            || !self.chrome_executable.is_absolute()
            || self.chrome_sha256.len() != 64
            || !self
                .chrome_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || self.initial_urls.is_empty()
            || self.initial_urls.len() > 16
            || self.principals.is_empty()
            || self.principals.len() > 8
        {
            return Err(io::Error::other("browser_policy_invalid"));
        }
        for u in &self.initial_urls {
            let allowed = u == "about:blank"
                || u.starts_with("https://")
                || u.starts_with("http://127.0.0.1:");
            if !allowed
                || u.len() > 2048
                || !u.is_ascii()
                || u.bytes()
                    .any(|b| b.is_ascii_control() || b == b'\\' || b == b'@')
            {
                return Err(io::Error::other("browser_initial_url_policy_invalid"));
            }
        }
        let mut pins = BTreeSet::new();
        for g in &self.principals {
            if !pins.insert(g.certificate.clone())
                || !config
                    .principals
                    .iter()
                    .any(|p| p.certificate == g.certificate)
                || (g.create && (!g.close || !g.observe))
            {
                return Err(io::Error::other("browser_principal_policy_invalid"));
            }
        }
        #[cfg(target_os = "macos")]
        if unsafe { libc::geteuid() } != self.user_uid || unsafe { libc::getuid() } != self.user_uid
        {
            return Err(io::Error::other("browser_user_binding_mismatch"));
        }
        Ok(())
    }
    pub fn grant(&self, p: &Principal) -> Option<&Grant> {
        // Actual authenticated certificate, not a legacy Native journal alias.
        self.principals
            .iter()
            .find(|g| g.certificate == p.certificate)
    }
}
