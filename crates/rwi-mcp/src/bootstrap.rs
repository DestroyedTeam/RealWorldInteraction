//! CLI target assembly and process lifecycle, independent from tool schemas.
#[cfg(feature = "native-agent-worker")]
use crate::agent_worker;
#[cfg(feature = "link-fixture")]
use crate::link_fixture;
use crate::{eof_diagnostics, server::Server};
use rmcp::ServiceExt;
use rwi_core::{Engine, Result as CoreResult, fault};
use std::path::PathBuf;
#[cfg(feature = "synthetic-fixture")]
fn synthetic_engine(args: &[String]) -> CoreResult<(Engine, Option<String>)> {
    // Deliberately no grant, native selector, pipe server, or default state path.
    if args.len() != 3 || args[0] != "--synthetic-only" || args[1] != "--state-dir" {
        return Err(fault(
            "synthetic_only_cli",
            "Use only --synthetic-only --state-dir ABS. This test build cannot register native targets.",
        ));
    }
    let state = PathBuf::from(&args[2]);
    if !state.is_absolute() {
        return Err(fault(
            "invalid_cli",
            "Synthetic state directory must be absolute.",
        ));
    }
    let mut engine = Engine::new(state.clone())?;
    #[cfg(feature = "link-fixture")]
    engine.register(
        "synthetic-only".into(),
        Box::new(link_fixture::Instrumented::open(state)?),
    )?;
    #[cfg(not(feature = "link-fixture"))]
    engine.register(
        "synthetic-only".into(),
        Box::new(rwi_core::fixture::Fixture::default()),
    )?;
    Ok((engine, None))
}

fn engine_from_args() -> CoreResult<(Engine, Option<String>)> {
    #[cfg(feature = "synthetic-fixture")]
    {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args.iter().any(|arg| arg == "--synthetic-only") {
            return synthetic_engine(&args);
        }
    }
    let mut args = std::env::args().skip(1);
    let mut state = None;
    let mut macos = None;
    let mut allow_input = false;
    let mut windows = false;
    let mut windows_grant = None;
    let mut lifecycle_grant_path = None;
    let mut pipe_server = None;
    #[cfg(feature = "native-agent-worker")]
    let mut agent_guard_path = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            #[cfg(feature = "native-agent-worker")]
            "--agent-guard" => {
                if agent_guard_path.is_some() {
                    return Err(fault(
                        "invalid_cli",
                        "--agent-guard must occur exactly once.",
                    ));
                }
                agent_guard_path = Some(PathBuf::from(args.next().ok_or_else(|| {
                    fault(
                        "invalid_cli",
                        "--agent-guard requires an absolute private worker spec.",
                    )
                })?));
            }
            "--state-dir" => {
                state = Some(PathBuf::from(args.next().ok_or_else(|| {
                    fault("invalid_cli", "--state-dir needs an absolute path.")
                })?))
            }
            "--macos-window" => {
                macos =
                    Some(args.next().ok_or_else(|| {
                        fault("invalid_cli", "--macos-window needs PID:CGWindowID.")
                    })?)
            }
            "--allow-input" => allow_input = true,
            "--windows-desktop" => windows = true,
            "--windows-input-grant" => {
                windows_grant = Some(PathBuf::from(args.next().ok_or_else(|| {
                    fault(
                        "invalid_cli",
                        "--windows-input-grant requires an absolute allocation JSON path.",
                    )
                })?));
            }
            "--windows-lifecycle-grant" => {
                lifecycle_grant_path = Some(PathBuf::from(args.next().ok_or_else(|| {
                    fault(
                        "invalid_cli",
                        "--windows-lifecycle-grant needs an absolute JSON path.",
                    )
                })?));
            }
            "--pipe-server" => {
                pipe_server = Some(args.next().ok_or_else(|| {
                    fault("invalid_cli", "--pipe-server requires a local run ID.")
                })?)
            }
            _ => {
                return Err(fault(
                    "invalid_cli",
                    "Use --state-dir PATH, optionally --macos-window PID:ID and --allow-input. Defaults to zero targets.",
                ));
            }
        }
    }
    #[cfg(feature = "native-agent-worker")]
    if let Some(path) = &agent_guard_path {
        agent_worker::validate_guard_cli(path, windows, pipe_server.as_deref(), macos.is_some())?;
        #[cfg(not(windows))]
        return Err(fault(
            "unsupported_platform",
            "The native-agent-worker route must run in the explicitly allocated interactive Windows session.",
        ));
    }
    let state = state
        .or_else(|| std::env::var_os("RCU_STATE_DIR").map(PathBuf::from))
        .unwrap_or_else(|| {
            let base = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("LOCALAPPDATA"))
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            base.join(".local/state/real-computer-use")
        });
    if !state.is_absolute() {
        return Err(fault(
            "invalid_cli",
            "State directory must be absolute and shared by all cooperating RWI servers for this user.",
        ));
    }
    if pipe_server.is_some() && !windows {
        return Err(fault(
            "invalid_cli",
            "--pipe-server requires an explicitly allocated --windows-desktop observation target.",
        ));
    }
    if windows_grant.is_some() && (!windows || !allow_input) {
        return Err(fault(
            "invalid_cli",
            "--windows-input-grant requires --windows-desktop and --allow-input.",
        ));
    }
    if lifecycle_grant_path.is_some() && !windows {
        return Err(fault(
            "invalid_cli",
            "Lifecycle grants require --windows-desktop; they do not implicitly enable input.",
        ));
    }
    if windows && allow_input && windows_grant.is_none() {
        return Err(fault(
            "input_grant_required",
            "Windows input requires an explicit root/window/native-identity/expiry allocation JSON.",
        ));
    }
    // The feature is a separate synthetic-only build, not an extra production
    // target selector. Reject before reading grants or constructing an OS adapter,
    // even when the caller omitted --synthetic-only.
    if cfg!(feature = "synthetic-fixture") && (windows || macos.is_some()) {
        return Err(fault(
            "synthetic_native_forbidden",
            "This synthetic-only test build cannot register any native target.",
        ));
    }
    #[cfg(all(windows, feature = "native-agent-worker"))]
    if let Some(path) = &agent_guard_path {
        agent_worker::install(agent_worker::WindowsGuard::load(path)?)?;
    }
    let mut engine = Engine::new(state)?;
    if windows {
        if macos.is_some() {
            return Err(fault(
                "invalid_cli",
                "Windows target cannot be combined with macOS.",
            ));
        }
        let grant = windows_grant
            .map(|p| {
                if !p.is_absolute() {
                    return Err(fault("invalid_cli", "Input grant path must be absolute."));
                }
                #[cfg(all(windows, feature = "native-agent-worker"))]
                if agent_worker::installed().is_some() {
                    return serde_json::from_slice::<rwi_windows::InputGrant>(
                        &agent_worker::private_grant(p, 16384)?,
                    )
                    .map_err(|_| {
                        fault(
                            "invalid_input_grant",
                            "Use the exact private input grant schema.",
                        )
                    });
                }
                use std::io::Read;
                let mut bytes = Vec::new();
                std::fs::File::open(p)?
                    .take(16385)
                    .read_to_end(&mut bytes)?;
                if bytes.len() > 16384 {
                    return Err(fault("invalid_input_grant", "Input grant exceeds 16 KiB."));
                }
                serde_json::from_slice::<rwi_windows::InputGrant>(&bytes).map_err(|_| {
                    fault(
                        "invalid_input_grant",
                        "Use exact grant schema; unknown fields are rejected.",
                    )
                })
            })
            .transpose()?;
        let lifecycle_grant = lifecycle_grant_path
            .map(|p| {
                if !p.is_absolute() {
                    return Err(fault(
                        "invalid_cli",
                        "Lifecycle grant path must be absolute.",
                    ));
                }
                #[cfg(all(windows, feature = "native-agent-worker"))]
                if agent_worker::installed().is_some() {
                    return serde_json::from_slice::<rwi_core::lifecycle::LifecycleGrant>(
                        &agent_worker::private_grant(p, 65536)?,
                    )
                    .map_err(|_| {
                        fault(
                            "invalid_lifecycle_grant",
                            "Use the exact private lifecycle grant schema.",
                        )
                    });
                }
                use std::io::Read;
                let mut bytes = Vec::new();
                std::fs::File::open(p)?
                    .take(65537)
                    .read_to_end(&mut bytes)?;
                if bytes.len() > 65536 {
                    return Err(fault(
                        "invalid_lifecycle_grant",
                        "Lifecycle grant exceeds 64 KiB.",
                    ));
                }
                serde_json::from_slice::<rwi_core::lifecycle::LifecycleGrant>(&bytes).map_err(
                    |_| {
                        fault(
                            "invalid_lifecycle_grant",
                            "Use exact lifecycle schema; unknown fields are rejected.",
                        )
                    },
                )
            })
            .transpose()?;
        #[cfg(windows)]
        {
            #[cfg(feature = "native-agent-worker")]
            let backend: Box<dyn rwi_core::Backend> = if let Some(guard) = agent_worker::installed()
            {
                let (mut grant, mut lifecycle_grant) = (grant, lifecycle_grant);
                guard.constrain_grants(&mut grant, &mut lifecycle_grant)?;
                Box::new(agent_worker::wrap(
                    rwi_windows::NativeWindows::new_with_grants(grant, lifecycle_grant)?,
                    guard.clone(),
                )?)
            } else {
                Box::new(rwi_windows::NativeWindows::new_with_grants(
                    grant,
                    lifecycle_grant,
                )?)
            };
            #[cfg(not(feature = "native-agent-worker"))]
            let backend: Box<dyn rwi_core::Backend> = Box::new(
                rwi_windows::NativeWindows::new_with_grants(grant, lifecycle_grant)?,
            );
            engine.register("windows-local".into(), backend)?;
        }
        #[cfg(not(windows))]
        {
            let _ = (grant, lifecycle_grant);
            return Err(fault(
                "unsupported_platform",
                "--windows-desktop must run inside the explicitly allocated interactive Windows session.",
            ));
        }
    }
    if let Some(selector) = macos {
        #[cfg(target_os = "macos")]
        {
            let (pid, window) = selector
                .split_once(':')
                .ok_or_else(|| fault("invalid_cli", "Expected PID:CGWindowID."))?;
            let pid = pid
                .parse()
                .map_err(|_| fault("invalid_cli", "PID must be numeric."))?;
            let window = window
                .parse()
                .map_err(|_| fault("invalid_cli", "CGWindowID must be numeric."))?;
            engine.register(
                "macos-local".into(),
                Box::new(rwi_macos::NativeMac::new(pid, window, allow_input)?),
            )?;
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (selector, allow_input);
            return Err(fault(
                "unsupported_platform",
                "macOS native target requested on another OS.",
            ));
        }
    } else if allow_input && !windows {
        return Err(fault(
            "invalid_cli",
            "--allow-input requires an explicitly pinned native target.",
        ));
    }
    Ok((engine, pipe_server))
}
pub(crate) fn run() {
    eof_diagnostics::phase("main_enter");
    let result = (|| -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (engine, pipe_server) = engine_from_args()?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(2)
            .enable_all()
            .build()?;
        eof_diagnostics::phase("runtime_block_on_begin");
        let result = runtime.block_on(async {
            #[cfg(windows)]
            if let Some(id) = pipe_server {
                #[cfg(feature = "native-agent-worker")]
                if let Some(guard) = agent_worker::installed() {
                    let (pipe, peer) = rwi_windows::pipe::accept_owned(
                        &id,
                        guard.owner(),
                        guard.connection_budget(),
                    )
                    .await?;
                    guard.bind_peer(peer)?;
                    let service = Server::new(engine)
                        .serve(eof_diagnostics::transport(pipe))
                        .await?;
                    eof_diagnostics::phase("service_wait_begin");
                    let result = service.waiting().await;
                    eof_diagnostics::phase("service_wait_returned");
                    result?;
                    return Ok::<_, Box<dyn std::error::Error>>(());
                }
                let pipe = rwi_windows::pipe::accept(&id).await?;
                let service = Server::new(engine)
                    .serve(eof_diagnostics::transport(pipe))
                    .await?;
                eof_diagnostics::phase("service_wait_begin");
                let result = service.waiting().await;
                eof_diagnostics::phase("service_wait_returned");
                result?;
                return Ok::<_, Box<dyn std::error::Error>>(());
            }
            #[cfg(not(windows))]
            let _ = pipe_server;
            let service = Server::new(engine)
                .serve(eof_diagnostics::transport(rmcp::transport::stdio()))
                .await?;
            eof_diagnostics::phase("service_wait_begin");
            let result = service.waiting().await;
            eof_diagnostics::phase("service_wait_returned");
            result?;
            Ok::<_, Box<dyn std::error::Error>>(())
        });
        eof_diagnostics::phase("runtime_block_on_returned");
        eof_diagnostics::phase("runtime_drop_begin");
        drop(runtime);
        eof_diagnostics::phase("runtime_drop_returned");
        result
    })();
    if let Err(e) = result {
        // GUI-subsystem Windows services may have no stderr handle. Preserve
        // bounded startup/transport diagnostics only in the explicit state dir.
        use std::io::Write;
        let args: Vec<_> = std::env::args().collect();
        if let Some(path) = args
            .windows(2)
            .find(|p| p[0] == "--state-dir")
            .map(|p| PathBuf::from(&p[1]))
            && path.is_absolute()
            && path.is_dir()
            && let Ok(mut log) = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(path.join("last-process-error.log"))
        {
            let _ = writeln!(
                log,
                "{}",
                e.to_string().chars().take(2048).collect::<String>()
            );
        }
        let _ = writeln!(std::io::stderr(), "{e}");
        std::process::exit(1);
    }
    eof_diagnostics::phase("main_return");
}
