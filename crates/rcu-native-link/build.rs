//! Declare privilege inheritance explicitly, including for the "setup" filename.
//! No installer heuristics or implicit elevation: Ordinary/Admin is a runtime
//! configuration checked against the actual launching token.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_LEGACY_WORKFLOW").is_some()
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        for binary in [
            "rcu-native-agent",
            "rcu-native-control",
            "rcu-native-relay",
            "rcu-native-setup",
        ] {
            println!("cargo:rustc-link-arg-bin={binary}=/MANIFEST:EMBED");
            println!(
                "cargo:rustc-link-arg-bin={binary}=/MANIFESTUAC:level='asInvoker' uiAccess='false'"
            );
        }
    }
}
