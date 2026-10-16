//! Independent task-goal verification after model resource release.
use super::control::{FILE_CAP, no_active_or_live, owned_failure};
use crate::{runtime::Runtime, sha, types::Stage};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{collections::BTreeMap, io};

pub(super) async fn oracle(runtime: &Runtime, previous: &str) -> io::Result<Value> {
    no_active_or_live(runtime).await?;
    runtime.begin(Stage::Oracle, previous)?; // Uses the reserved cleanup budget.
    let result = async {

        let directory = runtime.files()?;
        let mut rows = Vec::new();
        let mut all_match = true;
        for file in &runtime.approval.files {
            runtime.completion_budget_check(Stage::Oracle)?;
            let bytes = directory.read(&file.name, FILE_CAP)?;
            let hash = sha(&bytes); let matched = hash == file.expected_sha256;
            all_match &= matched;
            rows.push(json!({"name":file.name,"bytes":bytes.len(),"sha256":hash,"expected_sha256":file.expected_sha256,"matches":matched,"bytes_base64":STANDARD.encode(&bytes)}));
        }
        let data = json!({"files":rows,"all_match":all_match,"independent_of_uia":true,"source":"exact_private_manifest_file_handles"});
        let name = runtime.save_response(&data)?;
        let mut refs = BTreeMap::new(); refs.insert("files".into(), name);
        let mut proof = runtime.base_proof(); proof.file_oracle_verified = all_match; proof.detail = data;
        let receipt = runtime.finish(Stage::Oracle, proof, refs)?;
        Ok(json!({"stage":"oracle","stage_receipt":receipt,"complete":true,"file_oracle_verified":true,"native_goal_verified":false}))
    }.await;
    if let Err(error) = &result {
        owned_failure(runtime, &format!("oracle_owned_failure:{error}")).await;
    }
    result
}
