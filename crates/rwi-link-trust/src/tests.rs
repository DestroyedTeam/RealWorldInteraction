use super::*;
use crate::provision::Generation;
use serde::Serialize;
use std::time::{Duration as StdDuration, Instant};
use time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn generated(role: &str) -> (Generation, Identity) {
    let generation = Generation::new().expect("fresh private Rust generation");
    let identity = generation
        .identity(
            role,
            generation.now - Duration::minutes(1),
            generation.now + Duration::days(1),
        )
        .expect("fresh identity");
    (generation, identity)
}

fn error_class(error: &io::Error) -> &'static str {
    use rustls::{CertificateError as C, Error as E};
    match error
        .get_ref()
        .and_then(|source| source.downcast_ref::<E>())
    {
        Some(E::General(message)) if message == "paired_server_fingerprint_mismatch" => {
            "server_pin"
        }
        Some(E::General(message)) if message == "paired_client_fingerprint_mismatch" => {
            "client_pin"
        }
        Some(E::InvalidCertificate(C::Expired | C::ExpiredContext { .. })) => "expired_certificate",
        Some(E::InvalidCertificate(C::NotValidYet | C::NotValidYetContext { .. })) => {
            "future_certificate"
        }
        Some(E::InvalidCertificate(C::UnknownIssuer | C::BadSignature)) => {
            "certificate_chain_or_signature"
        }
        Some(E::InvalidCertificate(C::NotValidForName | C::NotValidForNameContext { .. })) => {
            "server_name"
        }
        Some(E::InvalidCertificate(C::InvalidPurpose | C::InvalidPurposeContext { .. })) => {
            "certificate_purpose"
        }
        Some(E::NoCertificatesPresented) => "missing_client_certificate",
        Some(E::PeerIncompatible(_)) => "protocol_version",
        Some(E::AlertReceived(_)) => "peer_alert",
        _ => "tls_or_io_rejected",
    }
}

#[derive(Debug, Serialize)]
struct SideReport {
    accepted: bool,
    protocol: Option<String>,
    handshake_kind: Option<String>,
    early_data_accepted: bool,
    category: &'static str,
}
impl SideReport {
    fn failure(error: io::Error) -> Self {
        Self {
            accepted: false,
            protocol: None,
            handshake_kind: None,
            early_data_accepted: false,
            category: error_class(&error),
        }
    }
}

async fn exchange(
    server: Arc<ServerConfig>,
    client: Arc<ClientConfig>,
    name: &str,
) -> (SideReport, SideReport) {
    let (a, b) = tokio::io::duplex(4096);
    let server = async {
        let result = async {
            let mut tls = tokio_rustls::TlsAcceptor::from(server).accept(a).await?;
            let mut bytes = [0; 4];
            tls.read_exact(&mut bytes).await?;
            if bytes != [10, 20, 30, 40] {
                return Err(invalid("application_byte_mismatch"));
            }
            tls.write_all(&[41, 31, 21, 11]).await?;
            tls.flush().await?;
            Ok(SideReport {
                accepted: true,
                protocol: tls.get_ref().1.protocol_version().map(|p| format!("{p:?}")),
                handshake_kind: tls.get_ref().1.handshake_kind().map(|p| format!("{p:?}")),
                early_data_accepted: false,
                category: "application_bytes_verified",
            })
        }
        .await;
        result.unwrap_or_else(SideReport::failure)
    };
    let client = async {
        let result = async {
            let name =
                ServerName::try_from(name.to_owned()).map_err(|_| invalid("bad_test_name"))?;
            let mut tls = tokio_rustls::TlsConnector::from(client)
                .connect(name, b)
                .await?;
            tls.write_all(&[10, 20, 30, 40]).await?;
            tls.flush().await?;
            let mut bytes = [0; 4];
            tls.read_exact(&mut bytes).await?;
            if bytes != [41, 31, 21, 11] {
                return Err(invalid("application_byte_mismatch"));
            }
            Ok(SideReport {
                accepted: true,
                protocol: tls.get_ref().1.protocol_version().map(|p| format!("{p:?}")),
                handshake_kind: tls.get_ref().1.handshake_kind().map(|p| format!("{p:?}")),
                early_data_accepted: tls.get_ref().1.is_early_data_accepted(),
                category: "application_bytes_verified",
            })
        }
        .await;
        result.unwrap_or_else(SideReport::failure)
    };
    tokio::time::timeout(StdDuration::from_secs(5), async {
        tokio::join!(server, client)
    })
    .await
    .expect("handshake and application exchange bounded")
}

#[tokio::test]
async fn fresh_memory_duplex_tls_handshake_matrix() {
    let generation = Generation::new().unwrap();
    let start = generation.now - Duration::minutes(1);
    let end = generation.now + Duration::days(1);
    let server = generation.identity("agent", start, end).unwrap();
    let controller = generation.identity("controller", start, end).unwrap();
    let model = generation.identity("model", start, end).unwrap();
    let unpaired = generation.identity("controller", start, end).unwrap();
    let expired_client = generation
        .identity(
            "controller",
            generation.now - Duration::days(2),
            generation.now - Duration::days(1),
        )
        .unwrap();
    let expired_server = generation
        .identity(
            "agent",
            generation.now - Duration::days(2),
            generation.now - Duration::days(1),
        )
        .unwrap();
    let future_client = generation
        .identity(
            "controller",
            generation.now + Duration::days(1),
            generation.now + Duration::days(2),
        )
        .unwrap();
    let (_, foreign_client) = generated("controller");
    let (_, foreign_server) = generated("agent");
    let server_pin = server.fingerprint();
    let normal_server = server
        .server_config(&[controller.fingerprint(), model.fingerprint()])
        .unwrap();
    let mut records = Vec::new();
    for name in [
        "controller",
        "model",
        "unpaired_client",
        "server_pin",
        "expired_client",
        "expired_server",
        "future_client",
        "wrong_client_ca",
        "wrong_server_ca",
        "missing_client",
        "wrong_dns",
        "wrong_client_purpose",
        "tls12_only",
    ] {
        let mut test_server = normal_server.clone();
        let mut test_client = controller.client_config(&server_pin).unwrap();
        let mut dns = SERVER_DNS_NAME;
        let expected_category = match name {
            "controller" => None,
            "model" => {
                test_client = model.client_config(&server_pin).unwrap();
                None
            }
            "unpaired_client" => {
                test_client = unpaired.client_config(&server_pin).unwrap();
                Some("client_pin")
            }
            "server_pin" => {
                test_client = controller.client_config(&unpaired.fingerprint()).unwrap();
                Some("server_pin")
            }
            "expired_client" => {
                test_server = server
                    .server_config(&[expired_client.fingerprint()])
                    .unwrap();
                test_client = expired_client.client_config(&server_pin).unwrap();
                Some("expired_certificate")
            }
            "expired_server" => {
                test_server = expired_server
                    .server_config(&[controller.fingerprint()])
                    .unwrap();
                test_client = controller
                    .client_config(&expired_server.fingerprint())
                    .unwrap();
                Some("expired_certificate")
            }
            "future_client" => {
                test_server = server
                    .server_config(&[future_client.fingerprint()])
                    .unwrap();
                test_client = future_client.client_config(&server_pin).unwrap();
                Some("future_certificate")
            }
            "wrong_client_ca" => {
                test_server = server
                    .server_config(&[foreign_client.fingerprint()])
                    .unwrap();
                // Trust the expected server, but present a leaf from the other CA.
                let mut foreign = foreign_client.client_config(&server_pin).unwrap();
                let local_roots = controller.roots().unwrap();
                let webpki = rustls::client::WebPkiServerVerifier::builder_with_provider(
                    local_roots,
                    Arc::new(rustls::crypto::ring::default_provider()),
                )
                .build()
                .unwrap();
                Arc::get_mut(&mut foreign)
                    .unwrap()
                    .dangerous()
                    .set_certificate_verifier(Arc::new(crate::verifier::PinnedServer {
                        webpki,
                        pin: server_pin.clone(),
                    }));
                test_client = foreign;
                Some("certificate_chain_or_signature")
            }
            "wrong_server_ca" => {
                // A correct expected pin never substitutes for a trusted chain.
                test_server = foreign_server
                    .server_config(&[controller.fingerprint()])
                    .unwrap();
                test_client = controller
                    .client_config(&foreign_server.fingerprint())
                    .unwrap();
                Some("certificate_chain_or_signature")
            }
            "missing_client" => {
                test_client = Arc::new(
                    ClientConfig::builder_with_provider(Arc::new(
                        rustls::crypto::ring::default_provider(),
                    ))
                    .with_protocol_versions(&[&rustls::version::TLS13])
                    .unwrap()
                    .with_root_certificates(controller.roots().unwrap())
                    .with_no_client_auth(),
                );
                Some("missing_client_certificate")
            }
            "wrong_dns" => {
                dns = "not-the-agent.local";
                Some("server_name")
            }
            "wrong_client_purpose" => {
                test_server = server
                    .server_config(std::slice::from_ref(&server_pin))
                    .unwrap();
                test_client = server.client_config(&server_pin).unwrap();
                Some("certificate_purpose")
            }
            "tls12_only" => {
                test_client = Arc::new(
                    ClientConfig::builder_with_provider(Arc::new(
                        rustls::crypto::ring::default_provider(),
                    ))
                    .with_protocol_versions(&[&rustls::version::TLS12])
                    .unwrap()
                    .with_root_certificates(controller.roots().unwrap())
                    .with_client_auth_cert(
                        vec![controller.certificate.clone()],
                        controller.key.clone_key(),
                    )
                    .unwrap(),
                );
                Some("protocol_version")
            }
            _ => unreachable!(),
        };
        let began = Instant::now();
        let (server_result, client_result) = exchange(test_server, test_client, dns).await;
        if let Some(expected) = expected_category {
            assert!(
                !server_result.accepted && !client_result.accepted,
                "{name}: {server_result:?} {client_result:?}"
            );
            assert!(
                server_result.category == expected || client_result.category == expected,
                "{name}: expected {expected}; {server_result:?} {client_result:?}"
            );
        } else {
            assert!(
                server_result.accepted && client_result.accepted,
                "{name}: {server_result:?} {client_result:?}"
            );
            assert_eq!(server_result.protocol.as_deref(), Some("TLSv1_3"));
            assert_eq!(client_result.protocol.as_deref(), Some("TLSv1_3"));
            assert_eq!(server_result.handshake_kind.as_deref(), Some("Full"));
            assert_eq!(client_result.handshake_kind.as_deref(), Some("Full"));
            assert!(!client_result.early_data_accepted);
        }
        records.push(serde_json::json!({"case":name,"server":server_result,"client":client_result,"elapsed_micros":began.elapsed().as_micros()}));
    }
    if let Some(path) = std::env::var_os("RCU_TRUST_EVIDENCE") {
        std::fs::create_dir_all(&path).unwrap();
        let report = serde_json::json!({"schema":"rcu.private_tls_matrix.v1", "execution":"memory_duplex_real_rustls_no_network", "server_pin":server_pin,
            "controller_pin":controller.fingerprint(),"model_pin":model.fingerprint(),"certificate_or_key_bytes_included":false,"cases":records});
        std::fs::write(
            PathBuf::from(path).join("trust-handshake-matrix.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
    }
}

#[tokio::test]
async fn no_early_data_tickets_or_resumption_across_two_actual_handshakes() {
    let (generation, server) = generated("agent");
    let client = generation
        .identity(
            "controller",
            generation.now - Duration::minutes(1),
            generation.now + Duration::days(1),
        )
        .unwrap();
    let server_config = server.server_config(&[client.fingerprint()]).unwrap();
    let client_config = client.client_config(&server.fingerprint()).unwrap();
    assert!(!client_config.enable_early_data);
    assert_eq!(server_config.max_early_data_size, 0);
    assert_eq!(server_config.send_tls13_tickets, 0);
    assert!(!server_config.session_storage.can_cache());
    assert!(!server_config.ticketer.enabled());
    assert!(!server_config.key_log.will_log("CLIENT_RANDOM"));
    assert!(!client_config.key_log.will_log("CLIENT_RANDOM"));
    let mut raw =
        rustls::ClientConnection::new(client_config.clone(), server_name(SERVER_DNS_NAME).unwrap())
            .unwrap();
    assert!(raw.early_data().is_none());
    for _ in 0..2 {
        let (s, c) = exchange(
            server_config.clone(),
            client_config.clone(),
            SERVER_DNS_NAME,
        )
        .await;
        assert!(s.accepted && c.accepted);
        assert_eq!(s.handshake_kind.as_deref(), Some("Full"));
        assert_eq!(c.handshake_kind.as_deref(), Some("Full"));
        assert!(!c.early_data_accepted);
    }
}

#[test]
fn pin_name_identity_and_importer_caps_fail_closed() {
    assert!(PeerPin::parse("").is_err());
    assert!(PeerPin::parse(&"A".repeat(64)).is_err());
    assert!(serde_json::from_str::<PeerPin>("\"bad\"").is_err());
    assert!(server_name("127.0.0.1").is_err());
    assert!(server_name("rcu-agent.local.").is_err());
    let (generation, identity) = generated("agent");
    assert!(identity.server_config(&[]).is_err());
    assert!(
        identity
            .server_config(&[identity.fingerprint(), identity.fingerprint()])
            .is_err()
    );
    let other = generation
        .identity(
            "controller",
            generation.now - Duration::minutes(1),
            generation.now + Duration::days(1),
        )
        .unwrap();
    assert!(
        Identity::from_der(
            identity.ca.to_vec(),
            identity.certificate.to_vec(),
            Zeroizing::new(other.key.secret_der().to_vec())
        )
        .is_err()
    );
    assert!(
        Identity::from_der(
            identity.ca.to_vec(),
            identity.certificate.to_vec(),
            Zeroizing::new(b"-----BEGIN PRIVATE KEY-----".to_vec())
        )
        .is_err()
    );
    assert!(
        Identity::from_der(
            Vec::new(),
            identity.certificate.to_vec(),
            Zeroizing::new(identity.key.secret_der().to_vec())
        )
        .is_err()
    );
    assert!(peer_sha(None).is_err());
    assert!(
        verify_peer(
            Some(std::slice::from_ref(&identity.certificate)),
            &other.fingerprint()
        )
        .is_err()
    );
    let paths = IdentityPaths::in_profile(Path::new("/approved/private-profile"));
    assert!(paths.load_with(|_, cap| Ok(vec![1; cap + 1])).is_err());
    let mut traversal = paths.clone();
    traversal.private_key = PathBuf::from("/approved/private-profile/../key.der");
    assert!(
        traversal
            .load_with(|_, _| panic!("must reject before reader"))
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn provisioning_is_fresh_private_no_overwrite_and_no_ca_key_file() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let first = base.join("first");
    let second = base.join("second");
    let a = provision(&first).unwrap();
    let b = provision(&second).unwrap();
    assert_ne!(a.ca_sha256, b.ca_sha256);
    assert_ne!(a.agent.certificate_sha256, b.agent.certificate_sha256);
    assert_ne!(a.controller.certificate_sha256, a.model.certificate_sha256);
    assert!(!a.ca_private_key_persisted);
    assert!(provision(&first).is_err());
    let names = std::fs::read_dir(&first)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(names.len(), 4);
    assert!(names.contains(&"public-manifest.json".into()));
    for role in ["agent", "controller", "model"] {
        let directory = first.join(role);
        assert_eq!(
            std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let identity = IdentityPaths::in_profile(&directory).load().unwrap();
        let expected = match role {
            "agent" => &a.agent,
            "controller" => &a.controller,
            _ => &a.model,
        };
        assert_eq!(identity.fingerprint(), expected.certificate_sha256);
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 3);
        for name in ["ca.der", "certificate.der", "private-key.der"] {
            assert_eq!(
                std::fs::metadata(directory.join(name))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn private_import_rejects_permissions_symlinks_hardlinks_and_oversize() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let root = base.join("profiles");
    provision(&root).unwrap();
    let paths = IdentityPaths::in_profile(&root.join("agent"));
    std::fs::set_permissions(&paths.private_key, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(paths.load().is_err());
    std::fs::set_permissions(&paths.private_key, std::fs::Permissions::from_mode(0o600)).unwrap();
    let hardlink = root.join("other-key.der");
    std::fs::hard_link(&paths.private_key, &hardlink).unwrap();
    assert!(paths.load().is_err());
    std::fs::remove_file(hardlink).unwrap();
    let original_key = paths.private_key.with_extension("saved");
    std::fs::rename(&paths.private_key, &original_key).unwrap();
    symlink(&original_key, &paths.private_key).unwrap();
    assert!(paths.load().is_err());
    std::fs::remove_file(&paths.private_key).unwrap();
    std::fs::rename(original_key, &paths.private_key).unwrap();
    symlink(root.join("agent"), root.join("alias")).unwrap();
    assert!(
        IdentityPaths::in_profile(&root.join("alias"))
            .load()
            .is_err()
    );
    std::fs::write(&paths.ca, vec![0; CERTIFICATE_CAP + 1]).unwrap();
    assert!(paths.load().is_err());
    std::fs::set_permissions(root.join("agent"), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(paths.load().is_err());
}
