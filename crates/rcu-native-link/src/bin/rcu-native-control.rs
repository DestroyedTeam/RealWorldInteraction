use rcu_native_link::{
    client, invalid,
    private::read_private,
    types::{ClientConfig, ControlRequest, Role},
};
use std::{io, path::PathBuf};
async fn run() -> io::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 4 || args[0] != "--config" || args[2] != "--request" {
        return Err(invalid(
            "usage: rcu-native-control --config ABS_PRIVATE_controller.json --request ABS_PRIVATE_request.json",
        ));
    }
    let path = PathBuf::from(&args[1]);
    let config: ClientConfig = serde_json::from_slice(&read_private(&path, 65536)?)?;
    if config.role != Role::Controller || path != config.root.join("controller.json") {
        return Err(invalid("fixed_controller_config_and_role"));
    }
    let request: ControlRequest =
        serde_json::from_slice(&read_private(&PathBuf::from(&args[3]), 65536)?)?;
    let result = client::control(&config, &request).await?;
    println!("{}", serde_json::to_string(&result)?);
    if result["ok"] != true {
        return Err(invalid("control_operation_not_successful"));
    }
    Ok(())
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
        eprintln!("native-control: {e}");
        std::process::exit(1);
    }
}
