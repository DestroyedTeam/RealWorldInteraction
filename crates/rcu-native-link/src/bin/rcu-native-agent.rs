use rcu_native_link::{invalid, runtime::Runtime, server};
#[cfg(not(windows))]
use rcu_native_link::{private::read_private, sha};
use std::{io, path::PathBuf};

fn self_image(runtime: &Runtime) -> io::Result<()> {
    let expected = &runtime.config.agent_image;
    #[cfg(windows)]
    {
        let proof = rcu_windows::agent_binding::current_owner_proof()
            .map_err(rcu_native_link::private::core_error)?;
        if !proof
            .process
            .image_path
            .eq_ignore_ascii_case(&expected.path.to_string_lossy())
            || proof.process.image_sha256 != expected.sha256
        {
            return Err(invalid("agent_current_retained_image_mismatch"));
        }
    }
    #[cfg(not(windows))]
    {
        if std::env::current_exe()?.canonicalize()? != expected.path
            || sha(&read_private(&expected.path, 128 * 1024 * 1024)?) != expected.sha256
        {
            return Err(invalid("agent_current_image_mismatch"));
        }
    }
    let name = if runtime.config.backend.is_windows() {
        "rcu-native-agent.exe"
    } else {
        "rcu-native-agent"
    };
    if expected.path.file_name().and_then(|s| s.to_str()) != Some(name) {
        return Err(invalid("fixed_agent_basename"));
    }
    Ok(())
}
async fn run() -> io::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--binding" {
        #[cfg(windows)]
        {
            let proof = rcu_windows::agent_binding::current_owner_proof()
                .map_err(rcu_native_link::private::core_error)?;
            let root = rcu_windows::agent_binding::fixed_agent_installation_root()
                .map_err(rcu_native_link::private::core_error)?;
            println!(
                "{}",
                serde_json::json!({"owner":proof,"fixed_installation_root":root,"window_capture_or_input_performed":false})
            );
            return Ok(());
        }
        #[cfg(not(windows))]
        return Err(invalid("binding_is_native_windows_only"));
    }
    if args.len() != 2 || args[0] != "--config" {
        return Err(invalid(
            "usage: rcu-native-agent --config ABS_PRIVATE_agent.json | --binding",
        ));
    }
    let path = PathBuf::from(&args[1]);
    let runtime = Runtime::load(&path)?;
    if path != runtime.config.install_root.join("agent.json") {
        return Err(invalid("fixed_agent_config_path"));
    }
    self_image(&runtime)?;
    runtime.check_native()?;
    // Fixtures are explicitly compiled and constrained to owned loopback tests.
    if !runtime.config.backend.is_windows() && !runtime.config.bind.ip().is_loopback() {
        return Err(invalid("fixture_non_loopback_listener_forbidden"));
    }
    let listener = tokio::net::TcpListener::bind(runtime.config.bind).await?;
    println!(
        "{}",
        serde_json::json!({"version":1,"listening":listener.local_addr()?,"task":runtime.approval.task,"approval_sha256":runtime.approval.digest(),"native_binding_verified":runtime.config.backend.is_windows(),"model_goal_verified":false})
    );
    server::serve(runtime, listener).await
}
fn main() {
    let result = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .and_then(|runtime| {
            let result = runtime.block_on(run());
            runtime.shutdown_timeout(std::time::Duration::from_secs(2));
            result
        });
    if let Err(e) = result {
        eprintln!("native-agent: {e}");
        std::process::exit(1);
    }
}
