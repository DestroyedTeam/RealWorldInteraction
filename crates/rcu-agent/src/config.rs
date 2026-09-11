use rcu_link_trust::{IdentityPaths, PeerPin};
use serde::{Deserialize, Serialize};
use std::{io, net::SocketAddr, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Principal {
    pub certificate: PeerPin,
    pub observe: bool,
    pub input: bool,
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
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlatformPolicy {
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
            || !rcu_core::device::valid_id(&self.device_id)
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
                    .any(|q| q.certificate == p.certificate)
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
        #[cfg(feature = "fixture")]
        if matches!(self.platform, PlatformPolicy::Fixture) && !self.bind.ip().is_loopback() {
            return Err(io::Error::other("fixture_requires_loopback"));
        }
        Ok(())
    }
}
