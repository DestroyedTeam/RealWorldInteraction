//! Deliberate local operator entry point. Never accepts or prints key material.
use std::{io, path::PathBuf};
fn main() {
    if let Err(error) = run() {
        eprintln!("rwi-link-renew: {error}");
        std::process::exit(1);
    }
}
fn run() -> io::Result<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let usage = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: rwi-link-renew provision ABS_NEW_BUNDLE | migrate ABS_OLD_BUNDLE ABS_NEW_BUNDLE | status ABS_BUNDLE | renew ABS_BUNDLE",
        )
    };
    let Some(action) = args.first().and_then(|s| s.to_str()) else {
        return Err(usage());
    };
    let paths = args.iter().skip(1).map(PathBuf::from).collect::<Vec<_>>();
    if paths.iter().any(|p| !p.is_absolute()) {
        return Err(usage());
    }
    let value = match (action, paths.as_slice()) {
        ("provision", [new]) => serde_json::to_value(rwi_link_trust::provision_renewable(new)?),
        ("migrate", [old, new]) => {
            serde_json::to_value(rwi_link_trust::migrate_renewable(old, new)?)
        }
        ("status", [root]) => serde_json::to_value(rwi_link_trust::renewal_status(root)?),
        ("renew", [root]) => serde_json::to_value(rwi_link_trust::renew_if_due(root)?),
        _ => return Err(usage()),
    }
    .map_err(io::Error::other)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&value).map_err(io::Error::other)?
    );
    Ok(())
}
