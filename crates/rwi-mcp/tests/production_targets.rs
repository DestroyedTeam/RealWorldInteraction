//! Default production targets and their Windows manifest are separate from the
//! explicitly enabled legacy local-worker diagnostics. No desktop is allocated.
use serde_json::Value;
use std::path::Path;
use std::process::Command;

#[test]
fn default_mcp_build_excludes_legacy_native_pipe_client() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let metadata = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--no-deps",
            "--locked",
            "--offline",
            "--format-version",
            "1",
        ])
        .current_dir(root)
        .output()
        .expect("read actual Cargo target metadata");
    assert!(
        metadata.status.success(),
        "{}",
        String::from_utf8_lossy(&metadata.stderr)
    );
    let metadata: Value = serde_json::from_slice(&metadata.stdout).expect("Cargo metadata JSON");
    let package = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "rwi-mcp")
        .unwrap();
    let client = package["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "rwi-pipe-client")
        .unwrap();
    assert_eq!(
        client["required-features"],
        serde_json::json!(["legacy-local"])
    );
    assert_eq!(package["features"]["default"], serde_json::json!([]));

    // Compile the real build script, then exercise its Windows/MSVC inputs on
    // any CI host. This checks emitted flags, not Windows linking or GUI use.
    let temp = tempfile::tempdir().expect("private build-script test directory");
    let binary = temp.path().join(if cfg!(windows) {
        "manifest-probe.exe"
    } else {
        "manifest-probe"
    });
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let compiled = Command::new(rustc)
        .arg("--edition=2021")
        .arg(root.join("build.rs"))
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("compile build script");
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let output = |legacy: bool| {
        let mut command = Command::new(&binary);
        command
            .env("CARGO_CFG_TARGET_OS", "windows")
            .env("CARGO_CFG_TARGET_ENV", "msvc")
            .env_remove("CARGO_FEATURE_LEGACY_LOCAL");
        if legacy {
            command.env("CARGO_FEATURE_LEGACY_LOCAL", "1");
        }
        let output = command.output().expect("run build script");
        assert!(output.status.success());
        String::from_utf8(output.stdout).expect("build script UTF-8")
    };
    let default = output(false);
    assert!(!default.contains("cargo:rustc-link-arg-bin=rwi-pipe-client="));
    assert_eq!(
        default.matches("cargo:rustc-link-arg-bin=rwi-mcp=").count(),
        2
    );
    assert!(default.contains("/MANIFESTUAC:level='asInvoker' uiAccess='false'"));
    let legacy = output(true);
    assert_eq!(
        legacy
            .matches("cargo:rustc-link-arg-bin=rwi-pipe-client=")
            .count(),
        2
    );
    assert!(legacy.contains("/MANIFESTUAC:level='asInvoker' uiAccess='false'"));
}
