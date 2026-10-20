//! One admitted, non-reconnecting model session; actual worker bytes only.
use crate::{
    control, frame, invalid, mcp, net,
    runtime::Runtime,
    sha,
    types::{Attach, ModelEnd, Stage},
    worker::{Read, Write},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, io, sync::Arc, time::Duration};
use tokio::{io::BufReader, sync::mpsc, task::JoinHandle};

enum Incoming {
    Client(io::Result<Option<Vec<u8>>>),
    Worker(io::Result<Option<Vec<u8>>>),
}
/// Cancellation of the owning connection closes its background I/O lanes too.
/// Normal completion additionally joins every lane below; aborted completion
/// remains HOLD, never resource-release proof.
struct OwnedTask<T>(JoinHandle<T>);
impl<T> Drop for OwnedTask<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}
fn reader(
    mut input: BufReader<Read>,
    cap: usize,
    client: bool,
    output: mpsc::Sender<Incoming>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            // The read is not cancellation safe halfway through a frame. It
            // lives in one task, and cancellation permanently closes this lane.
            let next = if client {
                frame::read_frame(&mut input, cap).await
            } else {
                frame::read_raw_frame(&mut input, cap).await
            };
            let end = !matches!(&next, Ok(Some(_)));
            let event = if client {
                Incoming::Client(next)
            } else {
                Incoming::Worker(next)
            };
            if output.send(event).await.is_err() || end {
                break;
            }
        }
    })
}
async fn abort_join<T>(handle: &mut JoinHandle<T>) -> bool {
    handle.abort();
    tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .is_ok()
}

/// Caller owns a successful Store::begin(Model), and is responsible for HOLD
/// and bounded owned-child cleanup on every error from this function.
pub async fn admitted(
    runtime: Arc<Runtime>,
    attach: &Attach,
    client_input: BufReader<Read>,
    client_output: Write,
) -> io::Result<(String, Write)> {
    let stage_spec = runtime.model_worker_spec()?;
    let input_grant = control::expand(
        &runtime,
        &runtime.approval.input_grant_template,
        &BTreeMap::new(),
        stage_spec.expires_unix_ms - crate::budget::MODEL_END_MS,
    )?;
    let mut worker = runtime
        .make_worker(Stage::Model, input_grant, Value::Null)
        .await?;
    let worker_output = std::mem::replace(
        &mut worker.output,
        BufReader::new(Box::new(tokio::io::empty())),
    );
    let (sender, mut incoming) = mpsc::channel(2);
    let mut client_reader = OwnedTask(reader(client_input, frame::INBOUND, true, sender.clone()));
    let mut worker_reader = OwnedTask(reader(worker_output, frame::OUTBOUND, false, sender));
    let (out, mut queued) = mpsc::channel::<Vec<u8>>(2);
    let mut writer = OwnedTask(tokio::spawn(async move {
        let mut output = client_output;
        while let Some(raw) = queued.recv().await {
            net::write_frame(&mut output, &raw, frame::OUTBOUND).await?;
        }
        Ok::<_, io::Error>(output)
    }));
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut ready = false;
    let result: io::Result<()> = async {
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    runtime.dispatch_allowed()?;
                    runtime.model_budget_check()?;
                    worker.budget.check()?;
                    worker.owned.verify_binding(&runtime.config, &runtime.approval)?;
                    if writer.0.is_finished() { return Err(invalid("model_stdout_writer_ended")); }
                }
                event = incoming.recv() => match event.ok_or_else(|| invalid("model_lanes_ended"))? {
                    Incoming::Client(raw) => {
                        let raw = raw?.ok_or_else(|| invalid("physical_model_eof_without_end_hold"))?;
                        let value: Value = serde_json::from_slice(&raw)?;
                        if value.get("model_end").is_some() {
                            let end: ModelEnd = serde_json::from_value(value)?;
                            if end.transport_version != crate::TRANSPORT_VERSION || !end.model_end || end.task != attach.task || end.nonce != attach.nonce || !ready {
                                return Err(invalid("model_end_binding_mismatch"));
                            }
                            let proof = worker.ledger.proof();
                            if !proof.requests_terminal || !proof.operations_terminal || !proof.resources_released || !proof.leases_released || proof.mutation_uncertain || !proof.model_goal_verified {
                                return Err(invalid("model_end_requires_terminal_resources_and_fresh_goal"));
                            }
                            runtime.dispatch_allowed()?;
                            worker.budget.check()?;
                            runtime.model_end()?;
                            break;
                        }
                        runtime.dispatch_allowed()?;
                        runtime.model_budget_check()?;
                        worker.budget.check()?;
                        worker.owned.verify_binding(&runtime.config, &runtime.approval)?;
                        if !ready && !matches!(value["method"].as_str(), Some("initialize" | "notifications/initialized" | "tools/list")) {
                            return Err(invalid("model_business_dispatch_before_actual_mcp_ready"));
                        }
                        let pending = match worker.ledger.request(&value) {
                            Ok(p) => p,
                            Err(e) => {
                                // No original worker response is invented; this
                                // explicitly labelled local denial was not sent.
                                if let Some(id) = value.get("id") {
                                    out.try_send(mcp::response(id.clone(), &e.to_string()))
                                        .map_err(|_| invalid("model_output_backpressure_hold"))?;
                                    runtime.journal()?.event("model_local_denial_not_forwarded", json!({"raw_sha256":sha(&raw),"reason":e.to_string()}))?;
                                    continue;
                                }
                                return Err(e);
                            }
                        };
                        if let Some(p) = &pending && p.mutation {
                            let request = p.args["request_id"].as_str().ok_or_else(|| invalid("model_mutation_request_id_missing"))?;
                            runtime.journal()?.reserve(request, &raw)?;
                        }
                        runtime.journal()?.event("model_request_before_forward", json!({"raw_sha256":sha(&raw),"bytes":raw.len(),"id":value["id"]}))?;
                        net::write_frame(&mut worker.input, &raw, frame::INBOUND).await?;
                    }
                    Incoming::Worker(raw) => {
                        let raw = raw?.ok_or_else(|| invalid("actual_worker_eof_unknown_hold"))?;
                        let blob=runtime.save_wire(&raw)?;
                        runtime.journal()?.event("actual_worker_raw_frame_before_validation", json!({"raw_sha256":sha(&raw),"bytes":raw.len(),"raw_blob":blob}))?;
                        let value: Value = serde_json::from_slice(&raw)?;
                        let pending = worker.ledger.reply(&value, &raw)?;
                        if !ready && pending.method == "tools/list" {
                            // Ledger has validated the actual unchanged worker
                            // version, instructions and all 19 schema digests.
                            let spec = runtime.model_ready()?;
                            worker.mark_model_ready(spec.clone())?;
                            ready = true;
                            runtime.journal()?.event("actual_model_ready_full_usable_budget", json!({"usable_ms":crate::budget::MODEL_USABLE_MS,"budget":spec,"setup_separate":true,"schema_sha256":mcp::schema_sha()}))?;
                            let mut control = serde_json::to_vec(&net::ModelReady {
                                transport_version: crate::TRANSPORT_VERSION, model_ready: true,
                                task: attach.task.clone(), nonce: attach.nonce.clone(),
                                usable_ms: crate::budget::MODEL_USABLE_MS,
                            })?;
                            control.push(b'\n');
                            out.try_send(control).map_err(|_| invalid("model_ready_backpressure_hold"))?;
                        }
                        out.try_send(raw).map_err(|_| invalid("model_output_backpressure_hold"))?;
                        // Preserve the real uncertain response, then prevent any
                        // future mutation. The task remains held even if a later
                        // read-only oracle finds the effect.
                        if worker.ledger.mutation_hold {
                            runtime.hold("actual_worker_mutation_uncertain")?;
                        }
                    }
                }
            }
        }
        Ok(())
    }.await;
    let readers_joined =
        abort_join(&mut client_reader.0).await & abort_join(&mut worker_reader.0).await;
    drop(out);
    let flushed = tokio::time::timeout(Duration::from_secs(6), &mut writer.0).await;
    let output = match flushed {
        Ok(Ok(Ok(output))) => Ok(output),
        Ok(Ok(Err(e))) => Err(e),
        Ok(Err(e)) => Err(invalid(&e.to_string())),
        Err(_) => {
            let _ = abort_join(&mut writer.0).await;
            Err(invalid("model_stdout_flush_cleanup_timeout"))
        }
    };
    // Always finish/reap the fixed owned worker, including admission/protocol
    // errors after it was started. Never terminate the user's application.
    let mut proof = worker.finish(&runtime.config, &runtime.approval).await?;
    proof.resources_released &= readers_joined && output.is_ok();
    if let Err(e) = &result {
        runtime.journal()?.event(
            "model_failed_owned_cleanup",
            json!({"reason":e.to_string(),"proof":proof}),
        )?;
    }
    result?;
    let output = output?;
    runtime.model_budget_check()?;
    let receipt = runtime.finish(Stage::Model, proof, BTreeMap::new())?;
    Ok((receipt, output))
}
