//! One compiled tool catalog for adapter and agent; never loaded from a test fixture.
use super::api::*;
use serde_json::Value;
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Empty {}
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    pub writes: bool,
}
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "computer.recover_input",
            description: "Explicitly reconcile an input resource domain returned by computer.targets. Requires input authorization, idle native writers and independent native key/button state. Retains every uncertain action and rejects all old operation replays; this does not verify or retry the business goal.",
            input_schema: serde_json::to_value(schemars::schema_for!(RecoverInputArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.targets",
            description: "List explicit targets, pinned identities and actual native capabilities. No discovery or wakeup of virtual machines.",
            input_schema: serde_json::to_value(schemars::schema_for!(Empty)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.observe",
            description: "Capture a native-size window PNG with explicit normalization metadata and a short-lived frame. No input; no resolution changes. Capture permission must already exist.",
            input_schema: serde_json::to_value(schemars::schema_for!(ObserveArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.capture_visual",
            description: "Read a lightweight visible-desktop image of the exact selected native window. max_edge=64..2048 (default 1024), no upscaling. feedback=exclude (default) suspends the device presenter only during pixel copy; include means do not actively exclude, not a promise that a marker appears. No UIA walk, activation, lease, frame, artifact, view or input reference. This visual_id cannot authorize input. Native worker/cancellation and pre/post identity/visibility/geometry checks still apply; pixels are not an atomic compositor transaction.",
            input_schema: serde_json::to_value(schemars::schema_for!(CaptureVisualArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.resolve",
            description: "Bind one semantic element, window-level placement locator (kind=window) or original-image pixel to a specific frame, native window and PID. Ambiguous/unknown mappings are rejected; kind=window yields a non-input reference for computer.move_window.",
            input_schema: serde_json::to_value(schemars::schema_for!(ResolveArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.claim",
            description: "Acquire a cross-process cooperative input lease for this exact desktop identity. TTL 100..60000 ms. Does not grant OS permissions or focus windows.",
            input_schema: serde_json::to_value(schemars::schema_for!(ClaimArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.act",
            description: "Dispatch one intent against a fresh native binding, lease and stable request_id. Windows needs an allocated input grant; Text/KeyChord need exact focused UIA element. Text: 1..1024 UTF-16 units, no control chars; Enter/Tab separate. Chords: Ctrl/Alt/Shift+key. Read capabilities for scroll units. Typed intent {drag:{waypoints:[{at_ms,offset:[dx,dy]}]}} performs one bounded device-paced left drag when pointer_drag.implemented is advertised; offsets use device physical pixels from the reference, not preview pixels. No automatic focus or replay; independently verify.",
            input_schema: serde_json::to_value(schemars::schema_for!(ActArgs)).unwrap(),
            writes: true,
        },
        ToolDefinition {
            name: "computer.pointer_feedback",
            description: "Visual-only overlay marker for the user: show or hide at a desktop point. It never dispatches input, never moves the real cursor, never focuses a window and cannot be mistaken for a goal; acts also update it automatically.",
            input_schema: serde_json::to_value(schemars::schema_for!(
                crate::device::api::PointerFeedbackArgs
            ))
            .unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.move_window",
            description: "Explicitly move the bound window onto a requested display (built_in and/or exact native id). Lease-scoped and idempotent: it never presses input, never focuses, never replays, and invalidates frames/references on this target. Unsupported platforms fail closed.",
            input_schema: serde_json::to_value(schemars::schema_for!(
                crate::device::api::MoveWindowArgs
            ))
            .unwrap(),
            writes: true,
        },
        ToolDefinition {
            name: "computer.act_checked",
            description: "Preferred model action: select one exact element ID from a <=120s observed frame, claim a lease, then dispatch once per request_id. The server checks the original window/document context and exact element against fresh native state immediately before single durable dispatch. No pixel/name locator, implicit focus, or replay; unknown/changed context fails closed. Does not relax legacy act's 10s TTL. Typed drag paths use the selected element as origin; inspect pointer_drag capability, keep the exact frame, and verify independently. Release; Text/KeyChord still require exact focused UIA.",
            input_schema: serde_json::to_value(schemars::schema_for!(CheckedActArgs)).unwrap(),
            writes: true,
        },
        ToolDefinition {
            name: "computer.lifecycle_preflight",
            description: "Read bounded native application/window inventory and coverage notes for one explicit lifecycle authorization. No lease or dispatch: never launches, activates, sends input, captures pixels or grants authority. Partial/incomplete coverage is not proof of absence; inspect notes and cleanup status before separately requesting launch.",
            input_schema: serde_json::to_value(schemars::schema_for!(LifecyclePreflightArgs))
                .unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.launch_application",
            description: "Launch one exact preauthorized application/argv/file set, once per authorization/run. Requires explicit lifecycle grant and lease. Returns bounded before/after native inventories and events: new_window_observed, reused_existing_window, launched_no_window, ambiguous or uncertain. Never equate PID or API acceptance with window/task success; no input grant. Observe 1..5000ms; no retry after reservation.",
            input_schema: serde_json::to_value(schemars::schema_for!(LaunchArgs)).unwrap(),
            writes: true,
        },
        ToolDefinition {
            name: "computer.activate_window",
            description: "Explicitly activate one exact authorized HWND/PID/birth/epoch from a <=10s native discovery. Requires separate lifecycle grant, lease, stable request_id and expected_foreground tuple. SetForegroundWindow once, verify native foreground, yield on user input/change; no retry, key injection or hidden activation in read-only bind. Focus observation is not an application goal or input authority.",
            input_schema: serde_json::to_value(schemars::schema_for!(ActivateArgs)).unwrap(),
            writes: true,
        },
        ToolDefinition {
            name: "computer.lifecycle_receipt",
            description: "Read immutable lifecycle reservation/result for this connection native identity and grant run. kind=launch or activation. Works without an active action lease; does not dispatch, focus or grant input. Interrupted/reserved results remain uncertain and forbidden to replay.",
            input_schema: serde_json::to_value(schemars::schema_for!(LifecycleReceiptArgs))
                .unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.verify",
            description: "Poll an independent window/PID/element-scoped predicate, not the input acknowledgement. Element values require element id. Optional request_id must match the exact action scope. Equality does not establish causation.",
            input_schema: serde_json::to_value(schemars::schema_for!(VerifyArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.release",
            description: "Release this connection's input lease. Never releases another connection's token.",
            input_schema: serde_json::to_value(schemars::schema_for!(ReleaseArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.artifact",
            description: "Read a bounded raw PNG byte range with SHA-256 and total size. No file writes or clipboard use. as_image requires the complete original PNG in one <=8 MiB range; partial bytes are never labeled as an image.",
            input_schema: serde_json::to_value(schemars::schema_for!(ArtifactArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.discover",
            description: "Discover windows in the allocated desktop: at most 8 windows / 12 KiB structured data per page. Prefer an exact pid or title_contains filter to avoid model-side long enumeration. Use next_cursor alone for the same cached result; complete=false is not absence. Read-only, no focus or VM wakeup.",
            input_schema: serde_json::to_value(schemars::schema_for!(TargetArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.bind_window",
            description: "Select an exact recently discovered PID/HWND/process epoch; popup/return transitions require native owner-chain evidence, never similar titles. Invalidates all old frames/references; does not focus or input.",
            input_schema: serde_json::to_value(schemars::schema_for!(BindArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.image",
            description: "Return a directly viewable mapped PNG preview or native-resolution tile from the preserved original. crop=[x,y,width,height] uses ORIGINAL pixels; max_edge=64..2048. Never labels downsampling as original success.",
            input_schema: serde_json::to_value(schemars::schema_for!(ImageArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.resolve_view",
            description: "Map pixels from a specific delivered preview/tile back to its original frame and native desktop before creating a fresh reference. Do not pass view pixels to computer.resolve.",
            input_schema: serde_json::to_value(schemars::schema_for!(ResolveViewArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.cancel",
            description: "Request cancellation using the exact service-issued operation_ref. Bare monitor/request IDs are not accepted. Acknowledgement does NOT prove native work stopped: inspect the execution status. Expired references never resolve to another execution. Never replay input.",
            input_schema: serde_json::to_value(schemars::schema_for!(OperationArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.operation",
            description: "Query with one tagged selector: execution + exact service-issued operation_ref, or request + device_id + durable request_id. No auto lookup or bare monitor IDs. Expired execution references never fall back to durable requests or newer contexts. A stale reference does not prove that work stopped or no effect occurred.",
            input_schema: serde_json::to_value(schemars::schema_for!(OperationQueryArgs)).unwrap(),
            writes: false,
        },
    ]
}
pub fn validate(tool: &str, args: &Value) -> Result<(), serde_json::Error> {
    match tool {
        "computer.recover_input" => {
            let _: RecoverInputArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.targets" => {
            let _: Empty = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.observe" => {
            let _: ObserveArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.capture_visual" => {
            let _: CaptureVisualArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.resolve" => {
            let _: ResolveArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.claim" => {
            let _: ClaimArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.act" => {
            let _: ActArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.act_checked" => {
            let _: CheckedActArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.lifecycle_preflight" => {
            let _: LifecyclePreflightArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.launch_application" => {
            let _: LaunchArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.activate_window" => {
            let _: ActivateArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.lifecycle_receipt" => {
            let _: LifecycleReceiptArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.verify" => {
            let _: VerifyArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.release" => {
            let _: ReleaseArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.artifact" => {
            let _: ArtifactArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.discover" => {
            let _: TargetArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.bind_window" => {
            let _: BindArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.pointer_feedback" => {
            let _: crate::device::api::PointerFeedbackArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.move_window" => {
            let _: crate::device::api::MoveWindowArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.image" => {
            let _: ImageArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.resolve_view" => {
            let _: ResolveViewArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.cancel" => {
            let _: OperationArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.operation" => {
            let _: OperationQueryArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        _ => Err(serde::de::Error::custom("unknown capability")),
    }
}

/// These operations only access already-cached data; no native provider or lease.
pub fn cached_read(tool: &str) -> bool {
    matches!(
        tool,
        "computer.artifact" | "computer.resolve" | "computer.resolve_view"
    )
}
pub fn executable(tool: &str) -> bool {
    definitions().iter().any(|d| d.name == tool)
        && !matches!(
            tool,
            "computer.targets" | "computer.operation" | "computer.cancel"
        )
        && !cached_read(tool)
}
