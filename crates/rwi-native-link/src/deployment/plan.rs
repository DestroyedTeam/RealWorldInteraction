//! Deterministic local plan generation from explicitly supplied native metadata.
//! No discovery, assumption of a fresh Notepad tab, or runtime operation occurs.
use crate::{
    budget::{BudgetSpec, now_ms},
    invalid, sha,
    types::{Approval, BackendMode, ControlTool, FileSpec, Stage, Step},
};
use rwi_core::lifecycle::{ApplicationIdentity, ObservedPackage};
use rwi_windows::agent_contract::SessionBinding;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, io};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotepadSeed {
    pub task: String,
    pub controller_host: String,
    pub route_vm_id: String,
    pub native_binding: SessionBinding,
    pub application: ApplicationIdentity,
    pub observed_applications: Vec<ApplicationIdentity>,
    pub observed_packages: Vec<ObservedPackage>,
    pub application_startup_effects_authorized: bool,
    pub initial_utf8: String,
    pub expected_utf8: String,
}
fn reference(stage: &str, step: &str) -> Value {
    json!({"$result":{"stage":stage,"step":step,"pointer":""}})
}
fn step(id: &str, tool: ControlTool, arguments: Value) -> Step {
    Step {
        id: id.into(),
        tool,
        arguments,
    }
}
pub fn notepad(seed: NotepadSeed) -> io::Result<Approval> {
    seed.native_binding
        .validate()
        .map_err(crate::private::core_error)?;
    if uuid::Uuid::parse_str(&seed.route_vm_id).is_err()
        || seed
            .task
            .bytes()
            .any(|b| !b.is_ascii_alphanumeric() && b != b'-')
        || !seed.application_startup_effects_authorized
        || seed.expected_utf8.is_empty()
        || seed.expected_utf8.encode_utf16().count() > 1024
        || !seed
            .application
            .executable
            .rsplit(['\\', '/'])
            .next()
            .is_some_and(|s| s.eq_ignore_ascii_case("notepad.exe"))
    {
        return Err(invalid(
            "explicit_notepad_plan_identity_route_goal_and_startup_consent_required",
        ));
    }
    let now = now_ms();
    let file = "allocated.txt";
    let mut permission = json!({"authorization_id":"launch-notepad","application":seed.application,"arguments":[{"$file":file}],"allowed_files":[{"path":{"$file":file},"sha256":sha(seed.initial_utf8.as_bytes())}],"observed_applications":seed.observed_applications,"observed_packages":seed.observed_packages,"allowed_existing_windows":[],"application_startup_effects_authorized":true});
    let common = json!({"run_id":seed.task,"route_vm_id":{"$guest_claim":true},"expected_identity":{"$native_identity":true},"expires_unix_ms":{"$expires":true},"launches":[],"activations":[]});
    let mut bootstrap = common.clone();
    bootstrap["launches"] = json!([permission.clone()]);
    permission["allowed_existing_windows"] =
        json!({"$window_keys":reference("bootstrap","preflight")});
    let mut preflight = common.clone();
    preflight["launches"] = json!([permission]);
    let mut activate = common;
    activate["activations"] = json!([{"authorization_id":"activate-notepad","window":{"$one_candidate":reference("launch","launch")}}]);
    let target = "windows-local";
    let claim = || {
        step(
            "claim",
            ControlTool::Claim,
            json!({"target":target,"ttl_ms":30000}),
        )
    };
    let release = || {
        step(
            "release",
            ControlTool::Release,
            json!({"lease":{"$lease":true}}),
        )
    };
    let check = || {
        step(
            "preflight",
            ControlTool::LifecyclePreflight,
            json!({"target":target,"authorization_id":"launch-notepad"}),
        )
    };
    let stages = BTreeMap::from([
        (
            Stage::Bootstrap,
            vec![step("targets", ControlTool::Targets, json!({})), check()],
        ),
        (Stage::Preflight, vec![check()]),
        (
            Stage::Launch,
            vec![
                claim(),
                step(
                    "launch",
                    ControlTool::LaunchApplication,
                    json!({"target":target,"lease":{"$lease":true},"authorization_id":"launch-notepad","request_id":format!("{}-launch",seed.task),"timeout_ms":5000,"operation_id":"native-launch"}),
                ),
                release(),
            ],
        ),
        (
            Stage::Activate,
            vec![
                step(
                    "discovery",
                    ControlTool::Catalog,
                    json!({"target":target,"activation_target":{"$one_candidate":reference("launch","launch")}}),
                ),
                claim(),
                step(
                    "activate",
                    ControlTool::ActivateWindow,
                    json!({"target":target,"lease":{"$lease":true},"authorization_id":"activate-notepad","request_id":format!("{}-activate",seed.task),"expected_foreground":{"$foreground_window":reference("current","discovery")},"operation_id":"native-activate"}),
                ),
                release(),
            ],
        ),
    ]);
    let input = json!({"run_id":seed.task,"route_vm_id":{"$guest_claim":true},"expected_identity":{"$native_identity":true},"root":{"$input_root":reference("activate","activate")},"expires_unix_ms":{"$expires":true}});
    let approval = Approval {
        version: 1,
        task: seed.task.clone(),
        controller_host: seed.controller_host,
        guest_claim: Some(seed.route_vm_id),
        native_binding: Some(seed.native_binding),
        budget: BudgetSpec {
            issued_unix_ms: now,
            expires_unix_ms: now + 3_600_000,
            maximum_elapsed_ms: 3_600_000,
            clock_drift_tolerance_ms: 1000,
        },
        request_prefix: format!("{}-", seed.task),
        files: vec![FileSpec {
            name: file.into(),
            initial_utf8: seed.initial_utf8,
            expected_sha256: sha(seed.expected_utf8.as_bytes()),
        }],
        expected_ui_value: seed.expected_utf8,
        stages,
        input_grant_template: input,
        lifecycle_grant_template: json!({"bootstrap":bootstrap,"preflight":preflight,"launch":preflight,"activate":activate}),
        fixture_control: Value::Null,
    };
    approval.validate(BackendMode::Windows)?;
    Ok(approval)
}
pub fn write_client_entry(mut config: crate::types::ClientConfig) -> io::Result<Value> {
    use crate::{private::Directory, types::Role};
    if config.version != 1 {
        return Err(invalid("client_config_version"));
    }
    let directory = Directory::open(&config.root)?;
    if config.role == Role::Controller {
        if config.entry_sha256.is_some() {
            return Err(invalid("controller_has_no_model_entry"));
        }
        directory.create_new("controller.json", &serde_json::to_vec(&config)?, 65536)?;
        return Ok(
            json!({"controller_config":config.root.join("controller.json"),"connection_started":false}),
        );
    }
    if config.entry_sha256.is_some() {
        return Err(invalid("new_entry_sha_is_computed_not_supplied"));
    }
    let path = config.root.join(".mcp.json");
    let bytes = serde_json::to_vec(
        &json!({"mcpServers":{crate::entry::SERVER_KEY:{"command":config.relay_image.path,"args":["--entry",path]}}}),
    )?;
    config.entry_sha256 = Some(sha(&bytes));
    directory.create_new(".mcp.json", &bytes, 65536)?;
    directory.create_new("client.json", &serde_json::to_vec(&config)?, 65536)?;
    crate::entry::load(&path)?;
    Ok(
        json!({"final_entry":path,"entry_sha256":config.entry_sha256,"exact_relay_image_sha256":config.relay_image.sha256,"connection_started":false,"global_settings_changed":false}),
    )
}
