use std::{io, path::PathBuf};

fn main() {
    if let Err(error) = run() {
        eprintln!("rwi-link-provision: {error}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--profile-dir")) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected --profile-dir ABSOLUTE_NEW_PRIVATE_DIRECTORY",
        ));
    }
    let destination = PathBuf::from(args.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing new private profile directory",
        )
    })?);
    if args.next().is_some() || !destination.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "only one absolute profile directory argument is accepted",
        ));
    }
    let manifest = rwi_link_trust::provision(&destination)?;
    // Intentionally serialize only public metadata, never Identity/PKCS8 bytes.
    println!(
        "{}",
        serde_json::to_string_pretty(&manifest).map_err(io::Error::other)?
    );
    Ok(())
}
