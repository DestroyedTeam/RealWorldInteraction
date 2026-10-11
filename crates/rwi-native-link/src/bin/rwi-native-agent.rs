// The resident agent is not an interactive console application. Setup/control
// remain console tools; redirected stdout/stderr are still available to launchers.
#![cfg_attr(windows, windows_subsystem = "windows")]

use rwi_native_link::{invalid, runtime::Runtime, server};
#[cfg(not(windows))]
use rwi_native_link::{private::read_private, sha};
use std::{io, path::PathBuf};

#[cfg(any(windows, test))]
fn process_selector(args: &[std::ffi::OsString]) -> io::Result<(u32, u64)> {
    if args.len() != 3 || args[0] != "--inspect-process" {
        return Err(invalid("usage: --inspect-process PID CREATION_FILETIME"));
    }
    let pid = args[1]
        .to_str()
        .ok_or_else(|| invalid("native_pid"))?
        .parse::<u32>()
        .map_err(|_| invalid("native_pid"))?;
    let expected_created = args[2]
        .to_str()
        .ok_or_else(|| invalid("native_process_birth"))?
        .parse::<u64>()
        .map_err(|_| invalid("native_process_birth"))?;
    if pid == 0 || expected_created == 0 {
        return Err(invalid("positive_pid_and_process_birth_required"));
    }
    Ok((pid, expected_created))
}

fn self_image(runtime: &Runtime) -> io::Result<()> {
    let expected = &runtime.config.agent_image;
    #[cfg(windows)]
    {
        let proof = rwi_windows::agent_binding::current_owner_proof()
            .map_err(rwi_native_link::private::core_error)?;
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
        "rwi-native-agent.exe"
    } else {
        "rwi-native-agent"
    };
    if expected.path.file_name().and_then(|s| s.to_str()) != Some(name) {
        return Err(invalid("fixed_agent_basename"));
    }
    Ok(())
}
async fn run() -> io::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    #[cfg(windows)]
    if args.first().is_some_and(|arg| arg == "--inspect-process") {
        let (pid, expected_created) = process_selector(&args)?;
        let identity = rwi_windows::agent_binding::inspect_process(pid, expected_created)
            .map_err(rwi_native_link::private::core_error)?;
        println!("{}", serde_json::to_string(&identity)?);
        return Ok(());
    }
    if args.len() == 2 && args[0] == "--inspect-window" {
        #[cfg(windows)]
        {
            use rwi_core::Backend;
            let pid = args[1]
                .to_str()
                .ok_or_else(|| invalid("native_pid"))?
                .parse::<u32>()
                .map_err(|_| invalid("native_pid"))?;
            if pid == 0 {
                return Err(invalid("native_pid"));
            }
            let mut windows = rwi_windows::NativeWindows::new_with_grants(None, None)
                .map_err(rwi_native_link::private::core_error)?;
            let result = windows
                .discover_with(&rwi_core::DiscoveryQuery {
                    pid: Some(pid),
                    ..Default::default()
                })
                .map_err(rwi_native_link::private::core_error)?;
            println!(
                "{}",
                serde_json::json!({"inspection":result,"window_capture_or_input_performed":false})
            );
            return Ok(());
        }
        #[cfg(not(windows))]
        return Err(invalid("inspection_is_native_windows_only"));
    }
    if args.len() == 1 && args[0] == "--binding" {
        #[cfg(windows)]
        {
            let proof = rwi_windows::agent_binding::current_owner_proof()
                .map_err(rwi_native_link::private::core_error)?;
            let root = rwi_windows::agent_binding::fixed_agent_installation_root()
                .map_err(rwi_native_link::private::core_error)?;
            println!(
                "{}",
                serde_json::json!({"owner":proof,"privilege":rwi_windows::agent_binding::current_token_privilege().map_err(rwi_native_link::private::core_error)?,"fixed_installation_root":root,"window_capture_or_input_performed":false})
            );
            return Ok(());
        }
        #[cfg(not(windows))]
        return Err(invalid("binding_is_native_windows_only"));
    }
    if args.len() != 2 || args[0] != "--config" {
        return Err(invalid(
            "usage: rwi-native-agent --config ABS_PRIVATE_agent.json | --binding | --inspect-process PID CREATION_FILETIME | --inspect-window PID",
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

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<std::ffi::OsString> {
        values.iter().map(std::ffi::OsString::from).collect()
    }
    #[test]
    fn inspection_selector_retains_explicit_pid_and_exact_birth() {
        assert_eq!(
            process_selector(&args(&["--inspect-process", "42", "134335198145995687"])).unwrap(),
            (42, 134335198145995687)
        );
    }
    #[test]
    fn inspection_never_falls_back_to_pid_only_or_another_mode() {
        for input in [
            vec!["--inspect-process"],
            vec!["--inspect-process", "42"],
            vec!["--inspect-window", "42", "7"],
            vec!["--inspect-process", "42", "7", "extra"],
        ] {
            assert!(process_selector(&args(&input)).is_err());
        }
    }
    #[test]
    fn inspection_rejects_zero_negative_overflow_and_noninteger_fields() {
        for (pid, birth) in [
            ("0", "7"),
            ("42", "0"),
            ("-1", "7"),
            ("42", "-7"),
            ("4294967296", "7"),
            ("42", "18446744073709551616"),
            ("42", "7.0"),
            ("NaN", "7"),
        ] {
            assert!(process_selector(&args(&["--inspect-process", pid, birth])).is_err());
        }
    }
    #[test]
    fn inspection_preserves_native_integer_width_without_truncation() {
        assert_eq!(
            process_selector(&args(&[
                "--inspect-process",
                "4294967295",
                "18446744073709551615"
            ]))
            .unwrap(),
            (u32::MAX, u64::MAX)
        );
    }
}
