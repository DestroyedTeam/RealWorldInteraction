# RCU private trust, 0.1.0-native-link.1

## Runtime APIs

- `IdentityPaths { ca, certificate, private_key }` is a strict local-config object. The three absolute files must be distinct siblings in one private profile. `IdentityPaths::in_profile(path)` selects `ca.der`, `certificate.der`, `private-key.der`.
- `IdentityPaths::load()` imports bounded DER (CA and leaf each <= 64 KiB, PKCS8 key <= 16 KiB). Unix retains a current-owner mode-0700 directory fd and uses `openat(O_NOFOLLOW|O_NONBLOCK)` plus fd metadata. Files must be current-owner, mode-0600-compatible, regular, single-link; symlink components, changes while reading, empty and over-cap files fail.
- `IdentityPaths::load_with(Fn(&Path, usize) -> io::Result<Vec<u8>>)` is the Windows integration point. The **caller must use the native agent's private SID/ACL/no-reparse HANDLE reader**. It is not an authorization or secure-storage substitute. Windows `load()` deliberately refuses unsupported default access.
- `PeerPin::parse(&str)` accepts exactly 64 lowercase hex characters. Its `Deserialize` applies this validation too. `.as_str()` reads the public fingerprint.
- `Identity::fingerprint()` / `ca_fingerprint()` return `PeerPin`.
- `Identity::client_config(&server_pin)` and `server_config(&[controller_pin, model_pin])` return `Arc<rustls::{ClientConfig,ServerConfig}>`. Empty, duplicate, or >8 client pins fail.
- `peer_sha(peer_certificates)` returns the authenticated leaf `PeerPin`; `verify_peer(certs, expected)` is an optional redundant boundary check.
- `server_name("rcu-agent.local")` is the only accepted production DNS name. TCP endpoints are selected by the parent's local manifest; this crate does not resolve, scan, listen, or connect.

Both TLS verifiers first perform the official WebPKI chain/time/purpose validation (and server DNS validation), then narrow success to the paired pin. TLS handshake signatures and supported schemes are delegated unchanged. The rustls custom-verifier builder's API name is `dangerous`; this implementation **does not fabricate success assertions and has no skip verifier**. TLS is configured only with TLS 1.3; no early data, tickets, session resumption, key-log output, default root store, fixture trust, or plaintext fallback. Test-only TLS 1.2 support creates a real downgrade-negative peer and is not a production enabled protocol.

Pair trust, controller/model role, current native identity, and per-task local approval remain separate: TLS pins are not task authority or host/guest attestation. Root integration owns transport deadlines, role/stage policy and native acceptance.

## Fresh provisioning

Run the Rust `rcu-link-provision` binary with exactly `--profile-dir ABSOLUTE_NEW_PRIVATE_DIRECTORY`. Parent directories must already exist and be canonical/owned. Use a location excluded from all source/evidence packages. Provisioning is a preparation step, not a model-window operation.

The Rust rcgen/ring generator creates a new OS-random P-256 CA and separately keyed `agent`, `controller`, and `model` profiles. Leaves expire after 72 hours; the private CA certificate after seven days, with a five-minute not-before margin. No CA signing private key is written. All three per-role directories are mode 0700 and files mode 0600 from creation; the loader reads identities back before the public manifest is committed. There is no overwrite or retry-in-place. A failure can leave a private incomplete directory; it is not silently deleted or reused.

Only `public-manifest.json` (pins, role names, validity timestamps) is printed and may be placed into evidence. Never archive `private-key.der` or whole private profile directories. Import **only the agent profile** into the Windows native user-private ACL directory; controller and model keys remain with their respective Mac clients. Import reviewed public pins separately into agent/relay approved config. No system-global trust is changed. Windows provisioning CLI is intentionally not implemented; the supported preparation flow generates on Mac and securely imports using the Windows native HANDLE/ACL layer.

Retained imported DER and rcgen's serialized key DER are zeroized on drop. Internal cryptographic-provider copies, heap snapshots, swap and protection against a hostile same-UID administrator are not attested by this component.

## Verification boundaries

`cargo test -p rcu-link-trust --locked --offline` runs fresh **memory-duplex** real rustls handshakes, not network or native operations: controller/model success, unpaired client, wrong server pin, expired client/server, not-yet-valid client, wrong client/server CA, missing client certificate, wrong DNS, wrong certificate purpose, and TLS-1.2-only peer refusal. Separate repeated full handshakes check early data/tickets/resumption disabled. Private-file, fresh/no-overwrite provisioning, and actual owned provisioning CLI tests do not contain retained test keys. `RCU_TRUST_EVIDENCE` optionally emits only public fingerprints, result categories and timing.

Root additionally owns real loopback TLS, full workspace fmt/test/clippy, Windows ARM64 build, production/fixture feature separation, and final independent verification. No Windows runtime or VM action was performed by this crate's tests.

Pinned dependency documentation: [rustls 0.23.41](https://docs.rs/rustls/0.23.41/rustls/), [rcgen 0.14.8](https://docs.rs/rcgen/0.14.8/rcgen/), [rcgen upstream](https://github.com/rustls/rcgen). Cargo.lock is root-owned and records official registry sources and checksums.
