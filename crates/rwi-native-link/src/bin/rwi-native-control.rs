use rwi_native_link::{
    client, invalid,
    private::read_private,
    types::{ClientConfig, ControlOp, ControlRequest, Role, Stage},
};
use std::{io, path::PathBuf};
async fn run() -> io::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let setup = args.len() == 3 && args[0] == "--config" && args[2] == "--start-model-setup";
    if !setup && (args.len() != 4 || args[0] != "--config" || args[2] != "--request") {
        return Err(invalid(
            "usage: rwi-native-control --config ABS_PRIVATE_controller.json (--request ABS_PRIVATE_request.json | --start-model-setup)",
        ));
    }
    let path = PathBuf::from(&args[1]);
    let config: ClientConfig = serde_json::from_slice(&read_private(&path, 65536)?)?;
    if config.role != Role::Controller || path != config.root.join("controller.json") {
        return Err(invalid("fixed_controller_config_and_role"));
    }
    if setup {
        let run = uuid::Uuid::new_v4().to_string();
        let mut previous = String::new();
        let mut responses = Vec::new();
        for (index, stage) in [
            Stage::Prepare,
            Stage::Bootstrap,
            Stage::Preflight,
            Stage::Launch,
            Stage::Activate,
        ]
        .into_iter()
        .enumerate()
        {
            let operation = if stage == Stage::Prepare {
                ControlOp::Prepare
            } else {
                ControlOp::RunStage {
                    stage,
                    previous_receipt: previous.clone(),
                }
            };
            let request = ControlRequest {
                version: 1,
                request_id: format!("setup-{run}-{index}"),
                operation,
            };
            let response = client::control(&config, &request).await?;
            let ok = response["ok"] == true;
            let valid_receipt = response["result"]["stage_receipt"]
                .as_str()
                .is_some_and(rwi_native_link::types::digest_shape);
            if let Some(receipt) = response["result"]["stage_receipt"].as_str() {
                previous = receipt.to_owned();
            }
            responses.push(serde_json::json!({"stage":stage,"response":response}));
            if !ok || !valid_receipt {
                println!(
                    "{}",
                    serde_json::json!({"ready_for_model_attachment":false,"automatic_retry":false,"stages":responses})
                );
                return Err(invalid("setup_stopped_on_first_failure_no_replay"));
            }
        }
        println!(
            "{}",
            serde_json::json!({"ready_for_model_attachment":true,"model_mcp_handshake_completed":false,"stage_receipt":previous,"stages":responses})
        );
        return Ok(());
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
