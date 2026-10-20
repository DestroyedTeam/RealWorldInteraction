use super::*;
use std::{
    os::unix::fs::{PermissionsExt, symlink},
    sync::Arc,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("renewable");
    (temp, root)
}

fn raw_reload(root: &Path, role: &str) -> Identity {
    let mut identity = IdentityPaths::in_profile(&root.join(role)).load().unwrap();
    identity.reload = Some(IdentityPaths::in_renewable_bundle(root, role).unwrap());
    identity
}

async fn handshake(
    server: Arc<rustls::ServerConfig>,
    client: Arc<rustls::ClientConfig>,
) -> io::Result<(PeerPin, PeerPin)> {
    let (a, b) = tokio::io::duplex(16384);
    let server = async {
        let mut stream = tokio_rustls::TlsAcceptor::from(server).accept(a).await?;
        let pin = crate::peer_sha(stream.get_ref().1.peer_certificates())?;
        let _ = stream.read_u8().await?;
        stream.write_u8(1).await?;
        stream.flush().await?;
        Ok::<_, io::Error>(pin)
    };
    let client = async {
        let mut stream = tokio_rustls::TlsConnector::from(client)
            .connect(crate::server_name(SERVER_DNS_NAME)?, b)
            .await?;
        let pin = crate::peer_sha(stream.get_ref().1.peer_certificates())?;
        stream.write_u8(1).await?;
        stream.flush().await?;
        let _ = stream.read_u8().await?;
        Ok::<_, io::Error>(pin)
    };
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        let (s, c) = tokio::join!(server, client);
        Ok((s?, c?))
    })
    .await
    .map_err(|_| invalid("test_handshake_deadline"))?
}

#[test]
fn explicit_pin_scheme_never_reinterprets_legacy_pins() {
    let generation = Generation::new().unwrap();
    let identity = generation
        .identity(
            "agent",
            generation.now - Duration::minutes(1),
            generation.now + Duration::days(1),
        )
        .unwrap();
    let spki = PeerPin::of_spki_der(identity.certificate.as_ref()).unwrap();
    assert_ne!(identity.fingerprint(), spki);
    assert!(spki.matches(identity.certificate.as_ref()).unwrap());
    assert!(PeerPin::parse(spki.as_str()).is_ok());
    for bad in [
        "spki:".to_owned(),
        format!("spki:{}", "A".repeat(64)),
        format!("spki:spki:{}", "a".repeat(64)),
    ] {
        assert!(PeerPin::parse(&bad).is_err());
    }
    let mut trailing = identity.certificate.to_vec();
    trailing.push(0);
    assert!(PeerPin::of_spki_der(&trailing).is_err());
    assert!(
        crate::authenticated_peer_pin(
            Some(std::slice::from_ref(&identity.certificate)),
            &[spki.clone(), identity.fingerprint()]
        )
        .is_err()
    );
    assert_eq!(
        crate::authenticated_peer_pin(
            Some(std::slice::from_ref(&identity.certificate)),
            std::slice::from_ref(&spki)
        )
        .unwrap(),
        spki
    );
}

#[test]
fn migration_preserves_all_endpoint_keys_but_replaces_ca_explicitly() {
    let (temp, root) = fixture();
    let old = temp.path().canonicalize().unwrap().join("legacy");
    let old_manifest = crate::provision(&old).unwrap();
    let migrated = migrate_renewable(&old, &root).unwrap();
    assert_ne!(old_manifest.ca_sha256, migrated.ca_sha256);
    assert!(migrated.ca_private_key_persisted);
    assert!(migrate_renewable(&old, &root).is_err());
    for role in ROLES {
        let before = IdentityPaths::in_profile(&old.join(role)).load().unwrap();
        let after = IdentityPaths::in_renewable_bundle(&root, role)
            .unwrap()
            .load()
            .unwrap();
        assert_eq!(before.key.secret_der(), after.key.secret_der());
        assert_eq!(
            PeerPin::of_spki_der(before.certificate.as_ref()).unwrap(),
            PeerPin::of_spki_der(after.certificate.as_ref()).unwrap()
        );
        assert_ne!(before.fingerprint(), after.fingerprint());
    }
    assert_eq!(
        std::fs::metadata(root.join("ca-private-key.der"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert!(renew_if_due(&root).unwrap().renewed.is_empty());
    let mut paths = IdentityPaths::in_renewable_bundle(&root, "model").unwrap();
    paths.certificate = old.join("model/certificate.der");
    assert!(paths.load().is_err());
}

#[tokio::test]
async fn same_tls_configs_accept_new_certificates_without_reload_or_restart() {
    let (_temp, root) = fixture();
    let now = OffsetDateTime::now_utc();
    let data = create(&root, None, now - Duration::days(151)).unwrap();
    let server = raw_reload(&root, "agent");
    let client = raw_reload(&root, "model");
    let server_config = server.server_config(&[data.peers[2].pin.clone()]).unwrap();
    let client_config = client.client_config(&data.peers[0].pin).unwrap();
    let before = handshake(server_config.clone(), client_config.clone())
        .await
        .unwrap();
    let report = renew_at(&root, now).unwrap();
    assert_eq!(report.renewed, ROLES);
    let after = handshake(server_config.clone(), client_config.clone())
        .await
        .unwrap();
    assert_ne!(before.0, after.0);
    assert_ne!(before.1, after.1);
    assert_eq!(report.leaves[0].certificate_sha256, after.1);
    assert_eq!(report.leaves[2].certificate_sha256, after.0);
    assert!(renew_at(&root, now).unwrap().renewed.is_empty());
    // Different endpoint key must not silently replace a retained signing key.
    let agent_path = root.join("agent/certificate.der");
    let original = std::fs::read(&agent_path).unwrap();
    Reader::open(&root.join("agent"))
        .unwrap()
        .replace(
            "certificate.der",
            &std::fs::read(root.join("model/certificate.der")).unwrap(),
        )
        .unwrap();
    assert!(
        handshake(server_config.clone(), client_config.clone())
            .await
            .is_err()
    );
    Reader::open(&root.join("agent"))
        .unwrap()
        .replace("certificate.der", &original)
        .unwrap();
    assert!(handshake(server_config, client_config).await.is_ok());
}

#[tokio::test]
async fn spki_does_not_bypass_expiry_purpose_ca_or_pairing() {
    let g = Generation::new().unwrap();
    let valid = |role| {
        g.identity(
            role,
            g.now - Duration::minutes(1),
            g.now + Duration::days(1),
        )
        .unwrap()
    };
    let server = valid("agent");
    let client = valid("model");
    let sp = PeerPin::of_spki_der(server.certificate.as_ref()).unwrap();
    let cp = PeerPin::of_spki_der(client.certificate.as_ref()).unwrap();
    let sc = server.server_config(std::slice::from_ref(&cp)).unwrap();
    let cc = client.client_config(&sp).unwrap();
    assert!(handshake(sc.clone(), cc.clone()).await.is_ok());
    let expired = g
        .identity(
            "agent",
            g.now - Duration::days(2),
            g.now - Duration::days(1),
        )
        .unwrap();
    let expired_pin = PeerPin::of_spki_der(expired.certificate.as_ref()).unwrap();
    assert!(
        handshake(
            expired.server_config(std::slice::from_ref(&cp)).unwrap(),
            client.client_config(&expired_pin).unwrap()
        )
        .await
        .is_err()
    );
    let future = g
        .identity(
            "model",
            g.now + Duration::days(1),
            g.now + Duration::days(2),
        )
        .unwrap();
    assert!(
        handshake(
            server
                .server_config(&[PeerPin::of_spki_der(future.certificate.as_ref()).unwrap()])
                .unwrap(),
            future.client_config(&sp).unwrap()
        )
        .await
        .is_err()
    );
    assert!(
        handshake(
            server.server_config(std::slice::from_ref(&sp)).unwrap(),
            server.client_config(&sp).unwrap()
        )
        .await
        .is_err()
    );
    assert!(
        handshake(sc.clone(), valid("model").client_config(&sp).unwrap())
            .await
            .is_err()
    );
    let foreign = Generation::new()
        .unwrap()
        .identity(
            "agent",
            g.now - Duration::minutes(1),
            g.now + Duration::days(1),
        )
        .unwrap();
    let fp = PeerPin::of_spki_der(foreign.certificate.as_ref()).unwrap();
    assert!(
        handshake(
            foreign.server_config(&[cp]).unwrap(),
            client.client_config(&fp).unwrap()
        )
        .await
        .is_err()
    );
}

#[test]
fn renewal_rejects_changed_keys_links_and_root_rollover() {
    let (_temp, root) = fixture();
    let now = OffsetDateTime::now_utc();
    create(&root, None, now - Duration::days(151)).unwrap();
    let source = root.join("ca-private-key.der");
    let saved = root.join("saved-key.der");
    std::fs::rename(&source, &saved).unwrap();
    symlink(&saved, &source).unwrap();
    assert!(renew_at(&root, now).is_err());
    std::fs::remove_file(&source).unwrap();
    std::fs::rename(&saved, &source).unwrap();
    let original = Zeroizing::new(std::fs::read(&source).unwrap());
    let other = Zeroizing::new(std::fs::read(root.join("model/private-key.der")).unwrap());
    Reader::open(&root)
        .unwrap()
        .replace("ca-private-key.der", &other)
        .unwrap();
    assert!(renew_at(&root, now).is_err());
    Reader::open(&root)
        .unwrap()
        .replace("ca-private-key.der", &original)
        .unwrap();
    assert!(renew_at(&root, now).is_ok());
    assert!(renew_at(&root, now + Duration::days(ROOT_DAYS)).is_err());
    assert!(
        renew_at(&root, now + Duration::days(ROOT_DAYS - 200))
            .unwrap_err()
            .to_string()
            .contains("root_rollover")
    );
}

#[test]
fn concurrent_due_checks_issue_only_once_and_preserve_private_files() {
    let (_temp, root) = fixture();
    let now = OffsetDateTime::now_utc();
    create(&root, None, now - Duration::days(151)).unwrap();
    let handles = (0..4)
        .map(|_| {
            let root = root.clone();
            std::thread::spawn(move || renew_at(&root, now))
        })
        .collect::<Vec<_>>();
    let reports = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect::<Vec<_>>();
    assert!(reports.iter().all(Result::is_ok), "{reports:?}");
    let reports = reports.into_iter().map(Result::unwrap).collect::<Vec<_>>();
    assert_eq!(reports.iter().filter(|r| !r.renewed.is_empty()).count(), 1);
    for report in &reports {
        assert_eq!(
            report.leaves[0].certificate_sha256,
            reports[0].leaves[0].certificate_sha256
        );
    }
    for role in ROLES {
        assert_eq!(
            std::fs::metadata(root.join(role).join("certificate.der"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}

#[test]
fn partial_role_update_finishes_without_changing_completed_role() {
    let (_temp, root) = fixture();
    let now = OffsetDateTime::now_utc();
    create(&root, None, now - Duration::days(151)).unwrap();
    let previous = ["controller", "model"].map(|r| {
        (
            r,
            std::fs::read(root.join(r).join("certificate.der")).unwrap(),
        )
    });
    let first = renew_at(&root, now).unwrap();
    for (role, certificate) in previous {
        Reader::open(&root.join(role))
            .unwrap()
            .replace("certificate.der", &certificate)
            .unwrap();
    }
    let completed = renew_at(&root, now).unwrap();
    assert_eq!(completed.renewed, ["controller", "model"]);
    assert_eq!(
        first.leaves[0].certificate_sha256,
        completed.leaves[0].certificate_sha256
    );
}

#[test]
fn startup_renews_expired_leaves_but_never_an_expired_root() {
    let (_temp, root) = fixture();
    let now = OffsetDateTime::now_utc();
    let manifest = create(&root, None, now - Duration::days(181)).unwrap();
    let old = renewal_status(&root).unwrap();
    assert!(
        old.leaves
            .iter()
            .all(|l| l.expires_unix_seconds < now.unix_timestamp())
    );
    let identity = IdentityPaths::in_renewable_bundle(&root, "agent")
        .unwrap()
        .load()
        .unwrap();
    let new = renewal_status(&root).unwrap();
    assert!(
        new.leaves
            .iter()
            .all(|l| l.expires_unix_seconds > now.unix_timestamp() + 179 * 86400)
    );
    assert_ne!(identity.fingerprint(), old.leaves[0].certificate_sha256);
    assert_eq!(new.leaves[0].pin, manifest.peers[0].pin);
    assert!(renew_at(&root, now + Duration::days(ROOT_DAYS)).is_err());
}
