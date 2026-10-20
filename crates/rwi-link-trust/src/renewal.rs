//! Opt-in local renewal, distinct from legacy CA-key-discarding provisioning.
//! Stable endpoint keys/SPKI pins; a private retained issuer; no system trust.
use crate::{
    CERTIFICATE_CAP, Identity, IdentityPaths, PRIVATE_KEY_CAP, PeerPin, SERVER_DNS_NAME, invalid,
    private_files::Reader,
    provision::{EphemeralSigner, Generation, issue_identity},
};
use rcgen::{Issuer, KeyPair};
use rustls::pki_types::CertificateDer;
use serde::{Deserialize, Serialize};
use std::{io, path::Path};
use time::{Duration, OffsetDateTime};
use x509_parser::{certificate::X509Certificate, prelude::FromDer};
use zeroize::Zeroizing;

const ROLES: [&str; 3] = ["agent", "controller", "model"];
const LEAF_DAYS: i64 = 180;
const RENEW_BEFORE_DAYS: i64 = 30;
const ROOT_DAYS: i64 = 3650;
const SCHEMA: &str = "rwi.renewable_pairing.v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenewablePeer {
    pub role: String,
    pub pin: PeerPin,
}

/// Immutable policy/identity record, not an expiry cache. Current leaf dates are
/// always read from the actual certificates, including after partial recovery.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenewalManifest {
    pub schema: String,
    pub server_dns_name: String,
    pub created_unix_seconds: i64,
    pub ca_sha256: PeerPin,
    pub ca_private_key_persisted: bool,
    pub peers: Vec<RenewablePeer>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LeafStatus {
    pub role: String,
    pub pin: PeerPin,
    pub certificate_sha256: PeerPin,
    pub not_before_unix_seconds: i64,
    pub expires_unix_seconds: i64,
    pub renew_at_unix_seconds: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenewalReport {
    pub schema: &'static str,
    pub ca_sha256: PeerPin,
    pub ca_expires_unix_seconds: i64,
    pub leaves: Vec<LeafStatus>,
    pub renewed: Vec<String>,
}

pub(crate) fn parse_certificate(der: &[u8]) -> io::Result<X509Certificate<'_>> {
    if der.is_empty() || der.len() > CERTIFICATE_CAP {
        return Err(invalid("trust_certificate_size_invalid"));
    }
    let (remaining, certificate) =
        X509Certificate::from_der(der).map_err(|_| invalid("trust_certificate_der_invalid"))?;
    if !remaining.is_empty() {
        return Err(invalid("trust_certificate_der_trailing_data"));
    }
    Ok(certificate)
}

pub(crate) fn validate_layout(bundle: &Path, paths: &IdentityPaths) -> io::Result<()> {
    let role = paths
        .ca
        .parent()
        .and_then(Path::file_name)
        .and_then(|s| s.to_str());
    let Some(role) = role.filter(|r| ROLES.contains(r)) else {
        return Err(invalid("trust_renewal_role_invalid"));
    };
    let expected = IdentityPaths::in_profile(&bundle.join(role));
    if !bundle.is_absolute()
        || paths.ca != expected.ca
        || paths.certificate != expected.certificate
        || paths.private_key != expected.private_key
    {
        return Err(invalid("trust_renewal_paths_not_exact_bundle_role"));
    }
    Ok(())
}

fn manifest(root: &Path) -> io::Result<RenewalManifest> {
    let bytes = Reader::open(root)?.read(&root.join("renewal.json"), 16384)?;
    let data: RenewalManifest =
        serde_json::from_slice(&bytes).map_err(|_| invalid("trust_renewal_manifest_invalid"))?;
    if data.schema != SCHEMA
        || data.server_dns_name != SERVER_DNS_NAME
        || !data.ca_private_key_persisted
        || data.peers.len() != ROLES.len()
        || ROLES
            .iter()
            .zip(&data.peers)
            .any(|(role, peer)| peer.role != *role || !peer.pin.as_str().starts_with("spki:"))
        || data
            .peers
            .iter()
            .enumerate()
            .any(|(i, peer)| data.peers[..i].iter().any(|p| p.pin == peer.pin))
    {
        return Err(invalid("trust_renewal_policy_or_identity_invalid"));
    }
    Ok(data)
}

pub(crate) fn validate_identity(paths: &IdentityPaths, identity: &Identity) -> io::Result<()> {
    let root = paths
        .renewal_bundle
        .as_ref()
        .ok_or_else(|| invalid("trust_renewal_bundle_missing"))?;
    validate_layout(root, paths)?;
    let role = paths
        .ca
        .parent()
        .and_then(Path::file_name)
        .and_then(|s| s.to_str())
        .ok_or_else(|| invalid("trust_renewal_role_invalid"))?;
    let data = manifest(root)?;
    let peer = data
        .peers
        .iter()
        .find(|p| p.role == role)
        .ok_or_else(|| invalid("trust_renewal_role_invalid"))?;
    check_identity(identity, &data.ca_sha256, &peer.pin)
}

fn check_identity(identity: &Identity, ca: &PeerPin, pin: &PeerPin) -> io::Result<()> {
    if identity.ca_fingerprint() != *ca || !pin.matches(identity.certificate.as_ref())? {
        return Err(invalid("trust_renewal_identity_changed"));
    }
    Ok(())
}

fn leaf_status(role: &str, identity: &Identity) -> io::Result<LeafStatus> {
    let certificate = parse_certificate(identity.certificate.as_ref())?;
    let end = certificate.validity().not_after.timestamp();
    Ok(LeafStatus {
        role: role.into(),
        pin: PeerPin::of_spki_der(identity.certificate.as_ref())?,
        certificate_sha256: identity.fingerprint(),
        not_before_unix_seconds: certificate.validity().not_before.timestamp(),
        expires_unix_seconds: end,
        renew_at_unix_seconds: end - Duration::days(RENEW_BEFORE_DAYS).whole_seconds(),
    })
}

/// New private bundle, or one-time migration copying only the three existing
/// endpoint keys. A new CA is required because legacy provisioning discarded it.
/// Destination is create-only; no config, ledger, Keychain or system trust edits.
pub fn provision_renewable(destination: &Path) -> io::Result<RenewalManifest> {
    create(destination, None, OffsetDateTime::now_utc())
}

pub fn migrate_renewable(source: &Path, destination: &Path) -> io::Result<RenewalManifest> {
    create(destination, Some(source), OffsetDateTime::now_utc())
}

fn create(
    destination: &Path,
    source: Option<&Path>,
    now: OffsetDateTime,
) -> io::Result<RenewalManifest> {
    let generation = Generation::at(now, ROOT_DAYS)?;
    let mut identities = Vec::new();
    let mut old_ca = None;
    for role in ROLES {
        let key = if let Some(source) = source {
            let old = IdentityPaths::in_profile(&source.join(role)).load()?;
            if old_ca
                .as_ref()
                .is_some_and(|ca| *ca != old.ca_fingerprint())
            {
                return Err(invalid("trust_migration_source_ca_mismatch"));
            }
            old_ca = Some(old.ca_fingerprint());
            Zeroizing::new(
                KeyPair::try_from(old.key.secret_der())
                    .map_err(|_| invalid("trust_migration_endpoint_key_invalid"))?,
            )
        } else {
            Zeroizing::new(
                KeyPair::generate()
                    .map_err(|_| invalid("trust_os_random_leaf_key_generation_failed"))?,
            )
        };
        identities.push(issue_identity(
            generation.issuer.der().to_vec(),
            &generation.issuer,
            role,
            now - Duration::minutes(5),
            now + Duration::days(LEAF_DAYS),
            key,
        )?);
    }
    let root = Reader::create(destination)?;
    root.write_new("renewal.lock", b"rwi-renewal-lock-v1\n")?;
    root.write_new("ca.der", generation.issuer.der().as_ref())?;
    let ca_key = Zeroizing::new(generation.issuer.key().0.serialize_der());
    root.write_new("ca-private-key.der", &ca_key)?;
    let mut peers = Vec::new();
    for (role, identity) in ROLES.into_iter().zip(identities) {
        let profile = Reader::create(&destination.join(role))?;
        profile.write_new("ca.der", identity.ca.as_ref())?;
        profile.write_new("certificate.der", identity.certificate.as_ref())?;
        profile.write_new("private-key.der", identity.key.secret_der())?;
        let loaded = IdentityPaths::in_profile(&destination.join(role)).load()?;
        if loaded.fingerprint() != identity.fingerprint()
            || loaded.ca_fingerprint() != identity.ca_fingerprint()
        {
            return Err(invalid("trust_renewal_private_readback_mismatch"));
        }
        peers.push(RenewablePeer {
            role: role.into(),
            pin: PeerPin::of_spki_der(identity.certificate.as_ref())?,
        });
    }
    let data = RenewalManifest {
        schema: SCHEMA.into(),
        server_dns_name: SERVER_DNS_NAME.into(),
        created_unix_seconds: now.unix_timestamp(),
        ca_sha256: PeerPin::of_der(generation.issuer.der().as_ref()),
        ca_private_key_persisted: true,
        peers,
    };
    root.write_new(
        "renewal.json",
        &serde_json::to_vec_pretty(&data).map_err(io::Error::other)?,
    )?;
    renewal_status(destination)?;
    Ok(data)
}

/// Read-only public status. Does not load/serialize the CA private key.
pub fn renewal_status(root: &Path) -> io::Result<RenewalReport> {
    let data = manifest(root)?;
    let ca = Reader::open(root)?.read(&root.join("ca.der"), CERTIFICATE_CAP)?;
    if PeerPin::of_der(&ca) != data.ca_sha256 {
        return Err(invalid("trust_renewal_ca_changed"));
    }
    let certificate = parse_certificate(&ca)?;
    if !certificate
        .basic_constraints()
        .map_err(|_| invalid("trust_renewal_ca_constraints_invalid"))?
        .is_some_and(|b| b.value.ca)
    {
        return Err(invalid("trust_renewal_issuer_not_ca"));
    }
    let mut leaves = Vec::new();
    for peer in &data.peers {
        let identity = IdentityPaths::in_profile(&root.join(&peer.role)).load()?;
        check_identity(&identity, &data.ca_sha256, &peer.pin)?;
        leaves.push(leaf_status(&peer.role, &identity)?);
    }
    Ok(RenewalReport {
        schema: "rwi.renewal_status.v1",
        ca_sha256: data.ca_sha256,
        ca_expires_unix_seconds: certificate.validity().not_after.timestamp(),
        leaves,
        renewed: Vec::new(),
    })
}

/// Renews only due leaves. Same key, same CA, same SPKI policy. Each certificate
/// is atomically replaced; an interrupted multi-role update is safe to finish on
/// the next check because mixed generations share the exact same trust policy.
pub fn renew_if_due(root: &Path) -> io::Result<RenewalReport> {
    renew_at(root, OffsetDateTime::now_utc())
}

fn renew_at(root: &Path, now: OffsetDateTime) -> io::Result<RenewalReport> {
    let directory = Reader::open(root)
        .map_err(|e| io::Error::new(e.kind(), format!("renewal_directory: {e}")))?;
    let _lock = directory
        .lock()
        .map_err(|e| io::Error::new(e.kind(), format!("renewal_lock: {e}")))?;
    let before = renewal_status(root)?;
    let ca = directory.read(&root.join("ca.der"), CERTIFICATE_CAP)?;
    let cert = parse_certificate(&ca)?;
    if now.unix_timestamp() < cert.validity().not_before.timestamp()
        || now.unix_timestamp() >= before.ca_expires_unix_seconds
    {
        return Err(invalid("trust_renewal_ca_expired_or_not_yet_valid"));
    }
    let due = before
        .leaves
        .iter()
        .filter(|leaf| now.unix_timestamp() >= leaf.renew_at_unix_seconds)
        .map(|leaf| leaf.role.clone())
        .collect::<Vec<_>>();
    if due.is_empty() {
        return Ok(before);
    }
    // Root rollover changes trust and always remains an explicit migration.
    if now + Duration::days(LEAF_DAYS)
        > OffsetDateTime::from_unix_timestamp(before.ca_expires_unix_seconds)
            .map_err(|_| invalid("trust_renewal_ca_time_invalid"))?
    {
        return Err(invalid("trust_renewal_root_rollover_required"));
    }
    let ca_key = Zeroizing::new(directory.read(&root.join("ca-private-key.der"), PRIVATE_KEY_CAP)?);
    // Checks the retained key really belongs to the paired CA, before issuing.
    Identity::from_der(ca.clone(), ca.clone(), Zeroizing::new(ca_key.to_vec()))?;
    let signer = EphemeralSigner(Zeroizing::new(
        KeyPair::try_from(ca_key.as_slice())
            .map_err(|_| invalid("trust_renewal_ca_key_invalid"))?,
    ));
    let ca_der = CertificateDer::from(ca.clone());
    let issuer = Issuer::from_ca_cert_der(&ca_der, signer)
        .map_err(|_| invalid("trust_renewal_ca_issuer_invalid"))?;
    // Validate and prepare all identities before making the first replacement.
    let mut replacements = Vec::new();
    for role in &due {
        let profile = root.join(role);
        let identity = IdentityPaths::in_profile(&profile).load()?;
        let key = Zeroizing::new(
            KeyPair::try_from(identity.key.secret_der())
                .map_err(|_| invalid("trust_renewal_endpoint_key_invalid"))?,
        );
        let renewed = issue_identity(
            ca.clone(),
            &issuer,
            role,
            now - Duration::minutes(5),
            now + Duration::days(LEAF_DAYS),
            key,
        )?;
        if PeerPin::of_spki_der(renewed.certificate.as_ref())?
            != PeerPin::of_spki_der(identity.certificate.as_ref())?
        {
            return Err(invalid("trust_renewal_spki_changed"));
        }
        replacements.push((profile, renewed));
    }
    for (profile, identity) in replacements {
        Reader::open(&profile)?.replace("certificate.der", identity.certificate.as_ref())?;
    }
    let mut after = renewal_status(root)?;
    after.renewed = due;
    Ok(after)
}

#[cfg(all(test, unix))]
#[path = "renewal_tests.rs"]
mod tests;
