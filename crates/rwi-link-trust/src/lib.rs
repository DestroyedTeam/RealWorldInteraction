//! Application-scoped, pinned mutual TLS 1.3 and fresh private provisioning.
//!
//! No certificate/key/trust bytes are compiled in; no system trust is modified.
//! The pin verifiers always call the complete official WebPKI verifier first.
//! A pin is an additional pairing check, never a substitute for chain, time,
//! purpose, DNS-name, or handshake-signature verification.

mod private_files;
mod provision;
mod renewal;
mod resolver;
mod verifier;

pub use private_files::Reader as PrivateDirectory;
pub use provision::{ProvisionManifest, ProvisionedPeer, provision};
pub use renewal::{
    RenewalManifest, RenewalReport, migrate_renewable, provision_renewable, renew_if_due,
    renewal_status,
};

/// Host-side bounded private configuration read, sharing the audited fd layer.
/// Windows callers supply their native ACL/HANDLE reader instead.
pub fn read_private(path: &std::path::Path, cap: usize) -> std::io::Result<Vec<u8>> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("private_parent_missing"))?;
    private_files::Reader::open(parent)?.read(path, cap)
}

use rustls::{
    ClientConfig, RootCertStore, ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName},
};
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};
use zeroize::Zeroizing;

pub const SERVER_DNS_NAME: &str = "rcu-agent.local";
pub const CERTIFICATE_CAP: usize = 64 * 1024;
pub const PRIVATE_KEY_CAP: usize = 16 * 1024;
pub const MAX_PAIRED_PEERS: usize = 8;

pub(crate) fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Legacy SHA-256 certificate pin, or explicitly prefixed `spki:` SHA-256 pin.
/// Existing unprefixed pins NEVER change meaning during a software update.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct PeerPin(String);

impl PeerPin {
    pub fn parse(value: &str) -> io::Result<Self> {
        let hash = value.strip_prefix("spki:").unwrap_or(value);
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(invalid("trust_pin_must_be_64_lowercase_hex"));
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn of_der(certificate: &[u8]) -> Self {
        Self(format!("{:x}", Sha256::digest(certificate)))
    }

    pub fn of_spki_der(certificate: &[u8]) -> io::Result<Self> {
        let parsed = renewal::parse_certificate(certificate)?;
        Ok(Self(format!(
            "spki:{:x}",
            Sha256::digest(parsed.public_key().raw)
        )))
    }

    pub fn matches(&self, certificate: &[u8]) -> io::Result<bool> {
        Ok(if self.0.starts_with("spki:") {
            Self::of_spki_der(certificate)? == *self
        } else {
            Self::of_der(certificate) == *self
        })
    }
}

impl<'de> Deserialize<'de> for PeerPin {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// File paths are local configuration, not wire messages or secret argv values.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityPaths {
    pub ca: PathBuf,
    pub certificate: PathBuf,
    pub private_key: PathBuf,
    /// Explicit opt-in to a local renewable pairing. No discovery of signing keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renewal_bundle: Option<PathBuf>,
}

impl IdentityPaths {
    pub fn in_renewable_bundle(bundle: &Path, role: &str) -> io::Result<Self> {
        let mut paths = Self::in_profile(&bundle.join(role));
        paths.renewal_bundle = Some(bundle.to_owned());
        paths.validate_layout()?;
        Ok(paths)
    }

    pub fn in_profile(profile: &Path) -> Self {
        Self {
            ca: profile.join("ca.der"),
            certificate: profile.join("certificate.der"),
            private_key: profile.join("private-key.der"),
            renewal_bundle: None,
        }
    }

    /// Unix uses a private owner directory HANDLE (fd) and no-follow openat.
    /// Windows must use `load_with` with the native SID/ACL/HANDLE file layer;
    /// there is deliberately no insecure std::fs or fixture-key fallback.
    pub fn load(&self) -> io::Result<Identity> {
        self.validate_layout()?;
        let reader = private_files::Reader::open(
            self.ca
                .parent()
                .ok_or_else(|| invalid("trust_profile_parent_missing"))?,
        )?;
        self.load_with(|path, cap| reader.read(path, cap))
    }

    /// The caller must supply a bounded private-file reader that rejects all
    /// reparse/symlink components and checks ownership, private ACL/permissions,
    /// and regular-file identity through the opened HANDLE. This exists for the
    /// Windows agent's common safe-file layer; this crate still checks lengths.
    pub fn load_with<F>(&self, read_private: F) -> io::Result<Identity>
    where
        F: Fn(&Path, usize) -> io::Result<Vec<u8>>,
    {
        self.validate_layout()?;
        if let Some(bundle) = &self.renewal_bundle {
            renew_if_due(bundle)?;
        }
        let ca = read_private(&self.ca, CERTIFICATE_CAP)?;
        let certificate = read_private(&self.certificate, CERTIFICATE_CAP)?;
        let key = Zeroizing::new(read_private(&self.private_key, PRIVATE_KEY_CAP)?);
        let mut identity = Identity::from_der(ca, certificate, key)?;
        if self.renewal_bundle.is_some() {
            renewal::validate_identity(self, &identity)?;
            identity.reload = Some(self.clone());
        }
        Ok(identity)
    }

    fn validate_layout(&self) -> io::Result<()> {
        let paths = [&self.ca, &self.certificate, &self.private_key];
        if paths.iter().any(|p| {
            !p.is_absolute()
                || p.components().any(|c| {
                    matches!(
                        c,
                        std::path::Component::CurDir | std::path::Component::ParentDir
                    )
                })
        }) || self.ca.parent() != self.certificate.parent()
            || self.ca.parent() != self.private_key.parent()
            || self.ca == self.certificate
            || self.ca == self.private_key
            || self.certificate == self.private_key
        {
            return Err(invalid(
                "trust_identity_paths_require_distinct_siblings_in_absolute_profile",
            ));
        }
        if let Some(bundle) = &self.renewal_bundle {
            renewal::validate_layout(bundle, self)?;
        }
        Ok(())
    }
}

/// Intentionally not Debug, Serialize, or Clone: private key contents must not
/// enter generic diagnostic/manifest output. The retained DER is zeroized on
/// drop; cryptographic-provider internal copies are owned by rustls/ring.
pub struct Identity {
    ca: CertificateDer<'static>,
    certificate: CertificateDer<'static>,
    key: Zeroizing<PrivateKeyDer<'static>>,
    reload: Option<IdentityPaths>,
}

impl Identity {
    fn from_der(ca: Vec<u8>, certificate: Vec<u8>, key: Zeroizing<Vec<u8>>) -> io::Result<Self> {
        if ca.is_empty()
            || ca.len() > CERTIFICATE_CAP
            || certificate.is_empty()
            || certificate.len() > CERTIFICATE_CAP
            || key.is_empty()
            || key.len() > PRIVATE_KEY_CAP
        {
            return Err(invalid("trust_der_empty_or_oversize"));
        }
        let parsed = PrivateKeyDer::try_from(key.as_slice())
            .map_err(|_| invalid("trust_private_key_not_der"))?;
        if !matches!(parsed, PrivateKeyDer::Pkcs8(_)) {
            return Err(invalid("trust_private_key_must_be_pkcs8"));
        }
        let identity = Self {
            ca: CertificateDer::from(ca),
            certificate: CertificateDer::from(certificate),
            key: Zeroizing::new(PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.to_vec()))),
            reload: None,
        };
        // Fail before network I/O if the certificate/key is malformed/mismatched.
        identity.roots()?;
        let provider = rustls::crypto::ring::default_provider();
        let signing_key = provider
            .key_provider
            .load_private_key(identity.key.clone_key())
            .map_err(|_| invalid("trust_pkcs8_signing_key_invalid"))?;
        rustls::sign::CertifiedKey::new(vec![identity.certificate.clone()], signing_key)
            .keys_match()
            .map_err(|_| invalid("trust_leaf_key_mismatch"))?;
        Ok(identity)
    }

    fn roots(&self) -> io::Result<Arc<RootCertStore>> {
        let mut roots = RootCertStore::empty();
        roots
            .add(self.ca.clone())
            .map_err(|_| invalid("trust_ca_der_invalid"))?;
        Ok(Arc::new(roots))
    }

    pub fn fingerprint(&self) -> PeerPin {
        PeerPin::of_der(self.certificate.as_ref())
    }

    pub fn ca_fingerprint(&self) -> PeerPin {
        PeerPin::of_der(self.ca.as_ref())
    }

    pub fn client_config(&self, server_pin: &PeerPin) -> io::Result<Arc<ClientConfig>> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let webpki = rustls::client::WebPkiServerVerifier::builder_with_provider(
            self.roots()?,
            provider.clone(),
        )
        .build()
        .map_err(|_| invalid("trust_server_verifier_invalid"))?;
        let pinned = verifier::PinnedServer {
            webpki,
            pin: server_pin.clone(),
        };
        // rustls calls this builder dangerous because it accepts arbitrary verifier
        // implementations. Ours performs WebPKI validation first, then *narrows*
        // the result to the locally paired pin; no assertion is fabricated.
        let mut config = ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(|_| invalid("trust_tls13_provider_invalid"))?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(pinned))
            .with_client_cert_resolver(Arc::new(resolver::Resolver::new(self)?));
        config.enable_early_data = false;
        config.resumption = rustls::client::Resumption::disabled();
        config.key_log = Arc::new(rustls::NoKeyLog {});
        Ok(Arc::new(config))
    }

    pub fn server_config(&self, allowed_clients: &[PeerPin]) -> io::Result<Arc<ServerConfig>> {
        if allowed_clients.is_empty()
            || allowed_clients.len() > MAX_PAIRED_PEERS
            || allowed_clients
                .iter()
                .enumerate()
                .any(|(i, pin)| allowed_clients[..i].contains(pin))
        {
            return Err(invalid("trust_client_pin_set_empty_duplicate_or_oversize"));
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let webpki = rustls::server::WebPkiClientVerifier::builder_with_provider(
            self.roots()?,
            provider.clone(),
        )
        .build()
        .map_err(|_| invalid("trust_client_verifier_invalid"))?;
        let pinned = verifier::PinnedClient {
            webpki,
            pins: allowed_clients.to_vec(),
        };
        let mut config = ServerConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(|_| invalid("trust_tls13_provider_invalid"))?
            .with_client_cert_verifier(Arc::new(pinned))
            .with_cert_resolver(Arc::new(resolver::Resolver::new(self)?));
        config.max_early_data_size = 0;
        config.send_tls13_tickets = 0;
        config.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});
        config.key_log = Arc::new(rustls::NoKeyLog {});
        Ok(Arc::new(config))
    }
}

pub fn server_name(value: &str) -> io::Result<ServerName<'static>> {
    if value != SERVER_DNS_NAME {
        return Err(invalid("trust_server_name_not_fixed_agent_name"));
    }
    ServerName::try_from(value.to_owned()).map_err(|_| invalid("trust_server_name_invalid"))
}

pub fn peer_sha(certs: Option<&[CertificateDer<'_>]>) -> io::Result<PeerPin> {
    certs
        .and_then(|c| c.first())
        .map(|c| PeerPin::of_der(c.as_ref()))
        .ok_or_else(|| invalid("trust_authenticated_peer_certificate_missing"))
}

pub fn verify_peer(certs: Option<&[CertificateDer<'_>]>, expected: &PeerPin) -> io::Result<()> {
    let certificate = certs
        .and_then(|c| c.first())
        .ok_or_else(|| invalid("trust_authenticated_peer_certificate_missing"))?;
    if !expected.matches(certificate.as_ref())? {
        return Err(invalid("trust_peer_pin_mismatch"));
    }
    Ok(())
}

/// Call only after the TLS verifier authenticated the chain and handshake.
/// Returns the configured pairing identity (stable across SPKI leaf renewal).
pub fn authenticated_peer_pin(
    certs: Option<&[CertificateDer<'_>]>,
    allowed: &[PeerPin],
) -> io::Result<PeerPin> {
    let certificate = certs
        .and_then(|c| c.first())
        .ok_or_else(|| invalid("trust_authenticated_peer_certificate_missing"))?;
    let mut matched = None;
    for pin in allowed {
        if pin.matches(certificate.as_ref())? {
            if matched.is_some() {
                return Err(invalid("trust_peer_pin_ambiguous"));
            }
            matched = Some(pin.clone());
        }
    }
    matched.ok_or_else(|| invalid("trust_peer_pin_mismatch"))
}

#[cfg(test)]
mod tests;
