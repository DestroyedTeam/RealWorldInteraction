use std::{io, path::PathBuf, time::Duration};

fn arguments() -> io::Result<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--entry")) {
        return Err(io::Error::other(
            "expected only --entry ABSOLUTE_FINAL_MCP_JSON",
        ));
    }
    let entry = PathBuf::from(
        args.next()
            .ok_or_else(|| io::Error::other("entry_path_missing"))?,
    );
    if args.next().is_some() || !entry.is_absolute() {
        return Err(io::Error::other(
            "only_absolute_final_entry_argument_allowed",
        ));
    }
    Ok(entry)
}

fn main() {
    let result = arguments().and_then(|entry| {
        let config = rwi_native_link::entry::load_current(&entry)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        let result = runtime.block_on(async {
            match rwi_native_link::net::relay_model(
                &config,
                tokio::io::stdin(),
                tokio::io::stdout(),
            )
            .await
            {
                Ok(receipt) => Ok(Some(receipt)),
                Err(error)
                    if error
                        .get_ref()
                        .is_some_and(|e| e.is::<rwi_native_link::net::StartupFailure>()) =>
                {
                    // This typed boundary guarantees no MCP input was consumed.
                    // Do not reconnect, forward, recover a task or grant input.
                    eprintln!("rwi-native-relay: startup_blocked_diagnostic_only: {error}");
                    rwi_native_link::integration::startup_diagnostic::serve(
                        &config.task,
                        &error.to_string(),
                    )
                    .await?;
                    Ok(None)
                }
                Err(error) => Err(error),
            }
        });
        // Tokio's stdin helper may retain an OS-blocked blocking-pool read when
        // the model keeps stdin open. Never let runtime Drop wait indefinitely.
        // The relay owns no worker process; its owned TLS/tasks are closed or
        // aborted in relay_model before this bounded shutdown/process exit.
        runtime.shutdown_timeout(Duration::from_millis(100));
        result
    });
    match result {
        Ok(Some(receipt)) => {
            eprintln!("rwi-native-relay: model_end_ack_verified stage_receipt={receipt}");
            std::process::exit(0);
        }
        Ok(None) => {
            eprintln!(
                "rwi-native-relay: diagnostic_server_closed; no native ModelEnd or goal claimed"
            );
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("rwi-native-relay: held_no_reconnect_or_replay: {error}");
            std::process::exit(1);
        }
    }
}
