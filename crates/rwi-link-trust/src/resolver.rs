//! Fresh certificate per handshake with a fixed in-memory endpoint signing key.
//! No fallback to a cached leaf when an explicitly renewable profile is invalid.
use crate::{
    CERTIFICATE_CAP, Identity, IdentityPaths, invalid, read_private, renew_if_due,
    renewal::parse_certificate,
};
use rustls::{
    SignatureScheme,
    client::ResolvesClientCert,
    server::{ClientHello, ResolvesServerCert},
    sign::CertifiedKey,
};
use std::{
    fmt, io,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(crate) struct Resolver {
    initial: Arc<CertifiedKey>,
    reload: Option<IdentityPaths>,
    checked: Mutex<Instant>,
    ca_not_before: i64,
    ca_not_after: i64,
}

impl fmt::Debug for Resolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PairedCertificateResolver")
            .field("renewable", &self.reload.is_some())
            .finish_non_exhaustive()
    }
}

impl Resolver {
    pub(crate) fn new(identity: &Identity) -> io::Result<Self> {
        let provider = rustls::crypto::ring::default_provider();
        let key = provider
            .key_provider
            .load_private_key(identity.key.clone_key())
            .map_err(|_| invalid("trust_signing_key_invalid"))?;
        let initial = CertifiedKey::new(vec![identity.certificate.clone()], key);
        initial
            .keys_match()
            .map_err(|_| invalid("trust_leaf_key_mismatch"))?;
        let ca = parse_certificate(identity.ca.as_ref())?;
        Ok(Self {
            initial: Arc::new(initial),
            reload: identity.reload.clone(),
            checked: Mutex::new(Instant::now()),
            ca_not_before: ca.validity().not_before.timestamp(),
            ca_not_after: ca.validity().not_after.timestamp(),
        })
    }

    fn current(&self) -> io::Result<Arc<CertifiedKey>> {
        let Some(paths) = &self.reload else {
            return Ok(self.initial.clone());
        };
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        if now < self.ca_not_before || now >= self.ca_not_after {
            return Err(invalid("trust_renewal_ca_expired_or_not_yet_valid"));
        }
        // Startup and the Agent's hourly worker are primary; this bounded check
        // also covers a long-sleeping MCP process before its next TLS handshake.
        let mut checked = self
            .checked
            .lock()
            .map_err(|_| invalid("trust_renewal_resolver_lock_poisoned"))?;
        if checked.elapsed() >= Duration::from_secs(3600) {
            renew_if_due(
                paths
                    .renewal_bundle
                    .as_ref()
                    .ok_or_else(|| invalid("trust_renewal_bundle_missing"))?,
            )?;
            *checked = Instant::now();
        }
        drop(checked);
        let certificate = read_private(&paths.certificate, CERTIFICATE_CAP)?;
        let key = CertifiedKey::new(vec![certificate.into()], self.initial.key.clone());
        key.keys_match()
            .map_err(|_| invalid("trust_renewal_leaf_key_changed"))?;
        Ok(Arc::new(key))
    }
}

impl ResolvesClientCert for Resolver {
    fn resolve(
        &self,
        _root_hint_subjects: &[&[u8]],
        schemes: &[SignatureScheme],
    ) -> Option<Arc<CertifiedKey>> {
        let key = self.current().ok()?;
        key.key.choose_scheme(schemes)?;
        Some(key)
    }
    fn has_certs(&self) -> bool {
        true
    }
}

impl ResolvesServerCert for Resolver {
    fn resolve(&self, hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let key = self.current().ok()?;
        key.key.choose_scheme(hello.signature_schemes())?;
        Some(key)
    }
}
