use rwi_link_trust::{IdentityPaths, PeerPin};
use serde::{Deserialize, Serialize};
use std::{io, net::SocketAddr, path::PathBuf};
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Principal {
    pub certificate: PeerPin,
    /// Explicit one-time legacy-to-SPKI migration continuity. Authentication
    /// still requires `certificate`; this only preserves the old journal scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub journal_identity: Option<PeerPin>,
    pub observe: bool,
    pub input: bool,
}
impl Principal {
    pub fn identity(&self) -> &PeerPin {
        self.journal_identity.as_ref().unwrap_or(&self.certificate)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u16,
    pub device_id: String,
    pub bind: SocketAddr,
    pub identity: IdentityPaths,
    pub principals: Vec<Principal>,
    pub state_root: PathBuf,
    pub platform: PlatformPolicy,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayPolicy {
    pub built_in: bool,
    #[serde(default)]
    pub id: Option<u32>,
    /// Off by default: implicit placement on bind is a side effect and must
    /// be opted in explicitly (incident 2026-09-15).
    #[serde(default)]
    pub place_on_bind: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlatformPolicy {
    #[serde(rename = "macos")]
    MacOs {
        user_uid: u32,
        /// Optional placement policy: only windows on a matching display may
        /// be bound. Omitted means every on-screen window stays selectable.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        authorized_display: Option<DisplayPolicy>,
    },
    Windows {
        user_sid: String,
        privilege: String,
    },
    Unsupported {
        platform: String,
    },
    #[cfg(feature = "fixture")]
    Fixture,
}
impl Config {
    pub fn validate(&self) -> io::Result<()> {
        if self.version != 1
            || !rwi_core::device::valid_id(&self.device_id)
            || self.principals.is_empty()
            || self.principals.len() > 8
            || !self.state_root.is_absolute()
            || self.bind.ip().is_multicast()
        {
            return Err(io::Error::other("invalid_device_service_config"));
        }
        for (i, p) in self.principals.iter().enumerate() {
            if (p.input && !p.observe)
                || self.principals[..i]
                    .iter()
                    .any(|q| q.certificate == p.certificate || q.identity() == p.identity())
                || p.journal_identity.as_ref().is_some_and(|old| {
                    !p.certificate.as_str().starts_with("spki:")
                        || old.as_str().starts_with("spki:")
                })
            {
                return Err(io::Error::other("invalid_principal_policy"));
            }
        }
        if let PlatformPolicy::Windows {
            user_sid,
            privilege,
        } = &self.platform
            && (!user_sid.starts_with("S-1-")
                || !matches!(privilege.as_str(), "ordinary" | "admin"))
        {
            return Err(io::Error::other("invalid_native_user_policy"));
        }
        if let PlatformPolicy::MacOs {
            authorized_display, ..
        } = &self.platform
            && authorized_display
                .as_ref()
                .is_some_and(|display| !display.built_in && display.id.is_none())
        {
            return Err(io::Error::other("invalid_display_policy"));
        }
        if matches!(
            self.platform,
            PlatformPolicy::MacOs {
                user_uid: u32::MAX,
                ..
            }
        ) {
            return Err(io::Error::other("invalid_native_user_policy"));
        }
        #[cfg(feature = "fixture")]
        if matches!(self.platform, PlatformPolicy::Fixture) && !self.bind.ip().is_loopback() {
            return Err(io::Error::other("fixture_requires_loopback"));
        }
        Ok(())
    }
}
