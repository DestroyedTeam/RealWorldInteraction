//! Model-facing cell summary; no native execution or lease cleanup.
use serde_json::{Value, json};

/// A failed cell is not a rollback. Summarize attempts separately from later
/// observation/local-JS failures without inventing application acceptance.
pub(super) fn execution_summary(calls: &[Value], failed: bool, cleanup_failed: bool) -> Value {
    let mut returned = 0;
    let mut rejected = 0;
    let mut unconfirmed = 0;
    for call in calls.iter().filter(|c| {
        matches!(
            c["tool"].as_str(),
            Some(
                "computer.act"
                    | "computer.act_checked"
                    | "computer.move_window"
                    | "computer.activate_window"
                    | "computer.launch_application"
            )
        )
    }) {
        if call["dispatch"] == "rejected_before_dispatch"
            && call["native_dispatch_entered"] == false
        {
            rejected += 1;
        } else if call["outcome"] == "returned"
            && call["is_error"] == false
            && call["dispatch"] == "returned"
            && call["dispatch_error"].is_null()
        {
            returned += 1;
        } else {
            // Missing/unknown evidence and preparation-only uncertainty remain
            // unconfirmed. Do not infer a no-effect outcome from an error flag.
            unconfirmed += 1;
        }
    }
    let failure = calls.iter().rev().find(|c| c["is_error"] == true);
    json!({
        "cell_status":if failed {"failed"} else {"returned"},
        "mutation_attempts":returned+rejected+unconfirmed,
        "dispatch_returned":returned,
        "rejected_before_dispatch":rejected,
        "dispatch_unconfirmed":unconfirmed,
        "last_failed_tool":failure.map(|c| &c["tool"]),
        "last_failed_code":failure.map(|c| &c["code"]),
        "cleanup_unconfirmed":cleanup_failed,
        "goal_verified":false,
        "replay_allowed":false,
        "next_step":if returned>0 || unconfirmed>0 {
            "Inspect original mutation call receipts and independently observe application state; a failed cell does not undo earlier dispatches. Never replay the cell."
        } else {
            "Inspect the error and obtain fresh observation/binding if needed; no automatic input retry."
        }
    })
}
