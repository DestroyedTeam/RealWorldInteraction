#![cfg_attr(windows, windows_subsystem = "windows")]
use std::{io, path::Path};
fn private_read(path: &Path, cap: usize) -> io::Result<Vec<u8>> {
    #[cfg(windows)]
    {
        rcu_windows::agent_binding::read_private_file_no_reparse(path, cap)
            .map_err(|e| io::Error::other(e.to_string()))
    }
    #[cfg(not(windows))]
    {
        rcu_link_trust::read_private(path, cap)
    }
}
async fn run() -> io::Result<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    #[cfg(windows)]
    if args.len() == 2 && args[0] == "--import-bundle" {
        println!("{}", rcu_agent::install::import(Path::new(&args[1]))?);
        return Ok(());
    }
    #[cfg(windows)]
    if args.len() == 1 && args[0] == "--describe-local" {
        let owner = rcu_windows::agent_binding::current_owner_proof()
            .map_err(|e| io::Error::other(e.to_string()))?;
        println!("{}", serde_json::to_string(&owner)?);
        return Ok(());
    }
    if args.len() != 2 || (args[0] != "--config" && args[0] != "--repair-namespace") {
        return Err(io::Error::other(
            "usage: rcu-agent --config ABS_PRIVATE_SERVICE_CONFIG | --repair-namespace ABS_PRIVATE_REPAIR_SPEC",
        ));
    }
    let repair: Option<rcu_agent::runtime::NamespaceRepair> = if args[0] == "--repair-namespace" {
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
    let config: rcu_agent::config::Config =
        serde_json::from_slice(&private_read(config_path, 65536)?)?;
    config.validate()?;
    #[cfg(windows)]
    let _state_guard = if config.state_root.exists() {
        rcu_windows::agent_binding::PrivateDirectory::open(&config.state_root)
    } else {
        rcu_windows::agent_binding::PrivateDirectory::create(&config.state_root)
    }
    .map_err(|e| io::Error::other(e.to_string()))?;
    #[cfg(unix)]
    let _state_guard = if config.state_root.exists() {
        rcu_link_trust::PrivateDirectory::open(&config.state_root)?
    } else {
        rcu_link_trust::PrivateDirectory::create(&config.state_root)?
    };
    if let Some(proof) = repair {
        let backup = private_read(&proof.committed_backup, 1024)?;
        println!(
            "{}",
            rcu_agent::runtime::Runtime::repair_namespace(&config, &proof, &backup)?
        );
        return Ok(());
    }
    let identity = config.identity.load_with(private_read)?;
    let tls = identity.server_config(
        &config
            .principals
            .iter()
            .map(|p| p.certificate.clone())
            .collect::<Vec<_>>(),
    )?;
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    let address = listener.local_addr()?;
    let runtime = rcu_agent::runtime::Runtime::new(config)?;
    if let Err(e) = rcu_agent::start_device_feedback(&runtime.config) {
        eprintln!(
            "rcu-agent feedback unavailable: {}: {}",
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
    let result = rcu_native_link::device::serve(
        listener,
        tls,
        move |peer, request| runtime.clone().handle(peer, request),
        rx,
    )
    .await;
    signal.abort();
    rcu_agent::stop_device_feedback();
    result
}
fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let result = runtime.block_on(run());
    runtime.shutdown_timeout(std::time::Duration::from_secs(2));
    if let Err(e) = result {
        eprintln!("rcu-agent: {e}");
        std::process::exit(1);
    }
}
