//! MCP workers inherit their explicitly allocated parent token; no auto-elevation.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        let mut binaries = vec!["rwi-mcp"];
        if std::env::var_os("CARGO_FEATURE_LEGACY_LOCAL").is_some() {
            binaries.push("rwi-pipe-client");
        }
        for binary in binaries {
            println!("cargo:rustc-link-arg-bin={binary}=/MANIFEST:EMBED");
            println!(
                "cargo:rustc-link-arg-bin={binary}=/MANIFESTUAC:level='asInvoker' uiAccess='false'"
            );
        }
    }
}
