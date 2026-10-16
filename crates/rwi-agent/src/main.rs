#![cfg_attr(windows, windows_subsystem = "windows")]
use std::{io, path::Path};
fn private_read(path: &Path, cap: usize) -> io::Result<Vec<u8>> {
    #[cfg(windows)]
    {
        rwi_windows::agent_binding::read_private_file_no_reparse(path, cap)
            .map_err(|e| io::Error::other(e.to_string()))
    }
    #[cfg(not(windows))]
    {
        rwi_link_trust::read_private(path, cap)
    }
}
async fn run() -> io::Result<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    #[cfg(windows)]
    if args.len() == 3 && args[0] == "--check-config" {
        return rwi_agent::install::check_config(Path::new(&args[1]), Path::new(&args[2]));
    }
    #[cfg(windows)]
    if args.len() == 3 && args[0] == "--describe-process" {
        let pid = args[1]
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or_else(|| io::Error::other("positive PID required"))?;
        let birth = args[2]
            .to_str()
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| io::Error::other("exact creation FILETIME required"))?;
        let status = rwi_windows::agent_binding::inspect_process(pid, birth)
            .map_err(|e| io::Error::other(e.to_string()))?;
        println!("{}", serde_json::to_string(&status)?);
        return Ok(());
    }
    #[cfg(windows)]
    if args.len() == 2 && args[0] == "--import-bundle" {
        println!("{}", rwi_agent::install::import(Path::new(&args[1]))?);
        return Ok(());
    }
    #[cfg(windows)]
    if args.len() == 1 && args[0] == "--describe-local" {
        let owner = rwi_windows::agent_binding::current_owner_proof()
            .map_err(|e| io::Error::other(e.to_string()))?;
        println!("{}", serde_json::to_string(&owner)?);
        return Ok(());
    }
    if args.len() != 2 || (args[0] != "--config" && args[0] != "--repair-namespace") {
        return Err(io::Error::other(
            "usage: rwi-agent --config ABS_PRIVATE_SERVICE_CONFIG | --repair-namespace ABS_PRIVATE_REPAIR_SPEC; Windows diagnostics: --describe-process PID CREATION_FILETIME | --check-config ABS_PRIVATE_CONFIG ABS_CREATE_ONLY_PRIVATE_REPORT",
        ));
    }
    let repair: Option<rwi_agent::runtime::NamespaceRepair> = if args[0] == "--repair-namespace" {
        Some(serde_json::from_slice(&private_read(
            Path::new(&args[1]),
            65536,
        )?)?)
    } else {
        None
    };
    if repair
        .as_ref()
        .is_some_and(|p| !p.config.is_absolute() || !p.committed_backup.is_absolute())
    {
        return Err(io::Error::other(
            "namespace repair requires absolute private config and backup paths",
        ));
    }
    let config_path = repair
        .as_ref()
        .map(|p| p.config.as_path())
        .unwrap_or_else(|| Path::new(&args[1]));
    let config: rwi_agent::config::Config =
        serde_json::from_slice(&private_read(config_path, 65536)?)?;
    config.validate()?;
    #[cfg(windows)]
    let _state_guard = if config.state_root.exists() {
        rwi_windows::agent_binding::PrivateDirectory::open(&config.state_root)
    } else {
        rwi_windows::agent_binding::PrivateDirectory::create(&config.state_root)
    }
    .map_err(|e| io::Error::other(e.to_string()))?;
    #[cfg(unix)]
    let _state_guard = if config.state_root.exists() {
        rwi_link_trust::PrivateDirectory::open(&config.state_root)?
    } else {
        rwi_link_trust::PrivateDirectory::create(&config.state_root)?
    };
    if let Some(proof) = repair {
        let backup = private_read(&proof.committed_backup, 1024)?;
        println!(
            "{}",
            rwi_agent::runtime::Runtime::repair_namespace(&config, &proof, &backup)?
        );
        return Ok(());
    }
    let identity = config.identity.load_with(private_read)?;
    let peers = config
        .principals
        .iter()
        .map(|p| p.certificate.clone())
        .collect::<Vec<_>>();
    let tls = identity.server_config(&peers)?;
    let renewal_bundle = config.identity.renewal_bundle.clone();
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    let address = listener.local_addr()?;
    let runtime = rwi_agent::runtime::Runtime::new(config)?;
    if let Err(e) = rwi_agent::start_device_feedback(&runtime.config) {
        eprintln!(
            "rwi-agent feedback unavailable: {}: {}",
            e.code, e.next_step
        );
    }
    println!(
        "{}",
        serde_json::json!({"service":"healthy","listening":address,"device_id":runtime.config.device_id,"instance":runtime.instance,"workflow_required":false})
    );
    let (stop, rx) = tokio::sync::watch::channel(false);
    let signal = tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = stop.send(true);
    });
    let renewal = renewal_bundle.map(|bundle| {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                let path = bundle.clone();
                match tokio::task::spawn_blocking(move || rwi_link_trust::renew_if_due(&path)).await
                {
                    Ok(Ok(report)) if !report.renewed.is_empty() => eprintln!(
                        "rwi-agent TLS renewed: {} (endpoint keys unchanged)",
                        report.renewed.join(",")
                    ),
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => eprintln!("rwi-agent TLS renewal failed: {error}"),
                    Err(_) => eprintln!("rwi-agent TLS renewal worker failed"),
                }
            }
        })
    });
    let result = rwi_native_link::device::serve_pinned(
        listener,
        tls,
        peers,
        move |peer, request| runtime.clone().handle(peer, request),
        rx,
    )
    .await;
    signal.abort();
    if let Some(renewal) = renewal {
        renewal.abort();
    }
    rwi_agent::stop_device_feedback();
    result
}
fn main() {
    #[cfg(target_os = "macos")]
    rwi_macos::overlay::install();
    let server = std::thread::Builder::new()
        .name("rwi-agent-server".into())
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .unwrap();
            let result = runtime.block_on(run());
            runtime.shutdown_timeout(std::time::Duration::from_secs(2));
            if let Err(e) = result {
                eprintln!("rwi-agent: {e}");
                return 1;
            }
            0
        })
        .expect("spawn rwi-agent-server thread");
    #[cfg(target_os = "macos")]
    {
        // AppKit overlay ownership: the main thread pumps commands while a
        // watcher exits the process once the server stops.
        std::thread::spawn(move || std::process::exit(server.join().unwrap_or(1)));
        rwi_macos::overlay::pump_forever();
    }
    #[cfg(not(target_os = "macos"))]
    std::process::exit(server.join().unwrap_or(1));
}
