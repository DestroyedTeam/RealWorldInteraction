use crate::{Identity, IdentityPaths, PeerPin, SERVER_DNS_NAME, invalid, private_files::Reader};
use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, DistinguishedName, DnType,
    ExtendedKeyUsagePurpose, IsCa, KeyPair, KeyUsagePurpose, PublicKeyData, SignatureAlgorithm,
    SigningKey,
};
use serde::{Deserialize, Serialize};
use std::{io, path::Path};
use time::{Duration, OffsetDateTime};
use zeroize::Zeroizing;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisionedPeer {
    pub role: String,
    /// Relative directory containing ca.der, certificate.der, private-key.der.
    pub directory: String,
    pub certificate_sha256: PeerPin,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisionManifest {
    pub schema: String,
    pub server_dns_name: String,
    pub created_unix_seconds: i64,
    pub expires_unix_seconds: i64,
    pub ca_sha256: PeerPin,
    pub agent: ProvisionedPeer,
    pub controller: ProvisionedPeer,
    pub model: ProvisionedPeer,
    pub ca_private_key_persisted: bool,
}

pub(crate) struct Generation {
    pub issuer: CertifiedIssuer<'static, EphemeralSigner>,
    pub now: OffsetDateTime,
}

/// Delegation only; key generation and signatures remain rcgen/ring. This
/// ensures rcgen's retained serialized DER is zeroized even on error paths.
pub(crate) struct EphemeralSigner(Zeroizing<KeyPair>);
impl PublicKeyData for EphemeralSigner {
    fn der_bytes(&self) -> &[u8] {
        self.0.der_bytes()
    }
    fn algorithm(&self) -> &'static SignatureAlgorithm {
        self.0.algorithm()
    }
}
impl SigningKey for EphemeralSigner {
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, rcgen::Error> {
        self.0.sign(message)
    }
}

impl Generation {
    pub fn new() -> io::Result<Self> {
        let now = OffsetDateTime::now_utc();
        let mut params = CertificateParams::new(Vec::<String>::new())
            .map_err(|_| invalid("trust_ca_parameters_invalid"))?;
        params.not_before = now - Duration::minutes(5);
        params.not_after = now + Duration::days(7);
        params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        params.distinguished_name = DistinguishedName::new();
        params
            .distinguished_name
            .push(DnType::CommonName, "RCU private run CA");
        let key = EphemeralSigner(Zeroizing::new(
            KeyPair::generate().map_err(|_| invalid("trust_os_random_ca_key_generation_failed"))?,
        ));
        let issuer = CertifiedIssuer::self_signed(params, key)
            .map_err(|_| invalid("trust_ca_certificate_generation_failed"))?;
        Ok(Self { issuer, now })
    }

    pub fn identity(
        &self,
        role: &str,
        not_before: OffsetDateTime,
        not_after: OffsetDateTime,
    ) -> io::Result<Identity> {
        let names = if role == "agent" {
            vec![SERVER_DNS_NAME.to_owned()]
        } else {
            Vec::new()
        };
        let mut params =
            CertificateParams::new(names).map_err(|_| invalid("trust_leaf_parameters_invalid"))?;
        params.not_before = not_before;
        params.not_after = not_after;
        params.is_ca = IsCa::ExplicitNoCa;
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![if role == "agent" {
            ExtendedKeyUsagePurpose::ServerAuth
        } else {
            ExtendedKeyUsagePurpose::ClientAuth
        }];
        params.distinguished_name = DistinguishedName::new();
        params
            .distinguished_name
            .push(DnType::CommonName, format!("RCU private {role}"));
        params.use_authority_key_identifier_extension = true;
        let key = Zeroizing::new(
            KeyPair::generate()
                .map_err(|_| invalid("trust_os_random_leaf_key_generation_failed"))?,
        );
        let certificate = params
            .signed_by(&*key, &self.issuer)
            .map_err(|_| invalid("trust_leaf_certificate_generation_failed"))?;
        Identity::from_der(
            self.issuer.der().to_vec(),
            certificate.der().to_vec(),
            Zeroizing::new(key.serialize_der()),
        )
    }
}

/// Generate an entirely new per-run private CA and three independently keyed
/// role identities using rcgen/ring OS randomness. No key is accepted in argv;
/// no CA private key is serialized, no system/global trust is touched.
///
/// The destination must not exist. A failure leaves a private partial directory
/// for explicit audit; it is not reused/overwritten. public-manifest.json is the
/// last committed file. Provisioning currently runs on Unix preparation hosts;
/// Windows receives only its private agent profile via the native ACL importer.
pub fn provision(destination: &Path) -> io::Result<ProvisionManifest> {
    let generation = Generation::new()?;
    let expiry = generation.now + Duration::days(3);
    let agent = generation.identity("agent", generation.now - Duration::minutes(5), expiry)?;
    let controller =
        generation.identity("controller", generation.now - Duration::minutes(5), expiry)?;
    let model = generation.identity("model", generation.now - Duration::minutes(5), expiry)?;
    let root = Reader::create(destination)?;
    let write_identity = |role: &str, identity: &Identity| -> io::Result<ProvisionedPeer> {
        let directory = destination.join(role);
        let profile = Reader::create(&directory)?;
        profile.write_new("ca.der", identity.ca.as_ref())?;
        profile.write_new("certificate.der", identity.certificate.as_ref())?;
        profile.write_new("private-key.der", identity.key.secret_der())?;
        let loaded = IdentityPaths::in_profile(&directory).load()?;
        if loaded.fingerprint() != identity.fingerprint()
            || loaded.ca_fingerprint() != identity.ca_fingerprint()
        {
            return Err(invalid("trust_provision_private_readback_mismatch"));
        }
        Ok(ProvisionedPeer {
            role: role.to_owned(),
            directory: role.to_owned(),
            certificate_sha256: identity.fingerprint(),
        })
    };
    let manifest = ProvisionManifest {
        schema: "rcu.private_pairing_provision.v1".into(),
        server_dns_name: SERVER_DNS_NAME.into(),
        created_unix_seconds: generation.now.unix_timestamp(),
        expires_unix_seconds: expiry.unix_timestamp(),
        ca_sha256: agent.ca_fingerprint(),
        agent: write_identity("agent", &agent)?,
        controller: write_identity("controller", &controller)?,
        model: write_identity("model", &model)?,
        ca_private_key_persisted: false,
    };
    // Exclude private-key files from all source/evidence packages. The public
    // manifest contains no key bytes and may be copied separately for pin review.
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|_| invalid("trust_provision_manifest_serialization_failed"))?;
    root.write_new("public-manifest.json", &bytes)?;
    Ok(manifest)
}
