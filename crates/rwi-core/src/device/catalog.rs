//! One compiled tool catalog for adapter and agent; never loaded from a test fixture.
mod browser;
use super::api::*;
use serde_json::Value;
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TargetsArgs {
    /// Probe only this paired device; omit to inspect every configured device.
    pub device_id: Option<String>,
}
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    pub writes: bool,
}
pub fn definitions() -> Vec<ToolDefinition> {
    let mut tools = browser::definitions();
    tools.extend([
        ToolDefinition {
            name: "computer.hotkey",
            description: "Explicit foreground/system keyboard input on an authorized macOS desktop, e.g. cmd+tab. Supply original desktop target, shared input lease, stable request_id and fresh expected_foreground from targets. Posts one bounded balanced key/modifier sequence into the system stream, not to a PID. Yields to recent HID activity or changed foreground/session; always releases its own posted keys. No text, auto-focus, background fallback or replay. Receipt is not a verified switch; independently observe foreground. Unsupported/scoped-window backends refuse. After dispatch processing, same-desktop old frames/refs in this executor are invalidated; snapshot then resolve before element input.",
            input_schema: serde_json::to_value(schemars::schema_for!(HotkeyArgs)).unwrap(),
            writes: true,
        },
        ToolDefinition {
            name: "computer.set_text",
            description: "Explicitly replace the entire writable CFString AXValue of one exact ordinary nonsecure macOS AXTextField/AXTextArea. Plain UTF-8 text (empty clears), original frame, stable request_id and internal lease. One guarded public AXValue setter, no focus/activation/keys/clipboard/append/automatic fallback/replay. Separate full AX value readback is not DOM, composition or keyboard goal. Unsupported/secure/unknown/specialized roles refuse.",
            input_schema: serde_json::to_value(schemars::schema_for!(SetTextArgs)).unwrap(),
            writes: true,
        },
        ToolDefinition {
            name: "computer.focus_element",
            description: "Explicit semantic focus of one exact plain editable element from the original frame. Internal device call requires lease and stable request_id; at most one AXFocused=true setter after fresh native policy checks. Never a click/action, window activation, value setter or keyboard fallback. Read back element focus, exact application focused element/window and OS foreground separately; acknowledgement alone is not success. Unsupported backends refuse; no retry/replay.",
            input_schema: serde_json::to_value(schemars::schema_for!(FocusElementArgs)).unwrap(),
            writes: true,
        },
        ToolDefinition {
            name: "computer.focus_window",
            description: "Explicitly bring one bound macOS visual window to foreground via public AX APIs, never a background-input fallback. Resolve kind=window from a <=10s frame; supply lease, stable request_id, and exact expected_foreground from targets/metadata. Guard identity/context/current foreground and HID activity. Each AX request once, no keys/warp/unhide/restore/retry. Receipt is not a goal or input grant; inspect fresh native foreground independently. Unsupported backends refuse.",
            input_schema: serde_json::to_value(schemars::schema_for!(FocusWindowArgs)).unwrap(),
            writes: true,
        },
        ToolDefinition {
            name: "computer.select_window_local",
            description: "Explicitly select this exact ordinary macOS visual window as app-local main or key. Fresh frame, kind main|key, durable request_id and internal lease; at most one requested attribute setter, no raise/frontmost/activation, element focus, keys, click, fallback or replay. Read exact window/application main/key CF axes and OS foreground separately; selected main is not key, editable readiness or typing authorization. Unsupported kind/provider/platform refuses.",
            input_schema: serde_json::to_value(schemars::schema_for!(WindowLocalArgs)).unwrap(),
            writes: true,
        },
        ToolDefinition {
            name: "computer.attach_window",
            description: "Create a NEW independent window target from the original desktop target and one fresh computer.discover selection (exact window/PID/native_epoch, relation=initial). Does not launch, activate, move, capture or replace the original binding/Live source. Use the returned target for snapshot/controls/actions. Windows are independently referenced but share the same physical desktop lease. Maximum 16 targets per context. Unsupported platforms report capability_unavailable.",
            input_schema: serde_json::to_value(schemars::schema_for!(BindArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.detach_window",
            description: "Retire one attached window target without closing its application/window. Release that target's lease first. Drops its cached frames/references and Live session; durable action receipts remain queryable and input is never replayed. Cannot detach the original desktop target; context close retires all bindings.",
            input_schema: serde_json::to_value(schemars::schema_for!(DetachWindowArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.recover_input",
            description: "Explicitly reconcile an input resource domain returned by computer.targets. Requires input authorization, idle native writers and independent native key/button state. Retains every uncertain action and rejects all old operation replays; this does not verify or retry the business goal.",
            input_schema: serde_json::to_value(schemars::schema_for!(RecoverInputArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.targets",
            description: "List explicit targets, pinned identities and actual native capabilities. Optional device_id probes only that paired endpoint, avoiding delays from unrelated offline devices; omit it to inspect all configured devices. No discovery or wakeup of virtual machines.",
            input_schema: serde_json::to_value(schemars::schema_for!(TargetsArgs)).unwrap(),
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
            description: "Read one bounded visual of the exact selected native window: Windows copies its visible desktop rectangle (other occlusion may appear); macOS uses a ScreenCaptureKit exact-window filter (separate panels are not included). max_edge=64..2048, no upscaling. feedback=exclude by default; inclusion does not promise a marker. No AX/UIA walk, activation, lease, frame, artifact, view or input reference. This visual_id cannot authorize input. Native pre/post identity and geometry checks apply; pixels are not an atomic compositor transaction.",
            input_schema: serde_json::to_value(schemars::schema_for!(CaptureVisualArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.live",
            description: "Control a bounded macOS Live overlay: start, hide, resume, pause, stop or status. Start needs options.display and a bound visual window. Open returned viewer.url via open_in_codex in this task; the overlay is hidden until that panel is visible. Switching tasks or covering/closing the panel hides and clears it. Follows explicit visual bindings; semantic-only dialogs do not change its source. Hide clears and removes the native panel until resume; pause clears but keeps a placeholder; stop/context close/expiry ends capture. No focus, input, screen-recording protection or native Host task-ID attestation. Inspect presenter status.",
            input_schema: serde_json::to_value(schemars::schema_for!(LiveArgs)).unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "computer.resolve",
            description: "Bind one semantic element, window-level placement locator (kind=window) or original-image pixel to a specific frame, native window and PID. Ambiguous/unknown mappings are rejected; kind=window yields a non-input reference for computer.move_window or explicit computer.focus_window.",
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
            description: "Dispatch one intent against a fresh native binding, lease and stable request_id. move_pointer sends one checked move without a button transition. Default uses the device native route. Optional strict JSON value: {delivery:foreground_system,expected_foreground:{pid,window,process_start,native_epoch}} requires pointer_move.foreground_system.implemented, exact current foreground and idle HID, and moves the system cursor; never automatic fallback. For move_pointer, macOS-only JSON value {delivery:background_skylight_experimental} requires pointer_move.background_skylight_experimental implemented+available; one private window-addressed move. For click only, the same explicit JSON selects pointer_click.background_skylight_experimental implemented+available: one left down/up pair, no AX action/double-click, activation, primer, auth or fallback. Plain AX action values retain exact marked-action checks. Dispatch does not prove an application effect. Windows needs an allocated input grant; Text/KeyChord need exact focused UIA element. Text: 1..1024 UTF-16 units, no control chars; Enter/Tab separate. Chords: Ctrl/Alt/Shift+key. Read capabilities for scroll units. Typed intent {drag:{waypoints:[{at_ms,offset:[dx,dy]}]}} performs one bounded device-paced left drag when pointer_drag.implemented is advertised; offsets use device physical pixels from the reference, not preview pixels. No automatic focus or replay; independently verify.",
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
            description: "Explicitly move the bound window onto a requested display (built_in, exact native id, and/or macOS ColorSync UUID). All supplied selectors must match one current display. Lease-scoped and idempotent: it never presses input, never focuses, never replays, and invalidates frames/references on this target. Unsupported platforms fail closed.",
            input_schema: serde_json::to_value(schemars::schema_for!(
                crate::device::api::MoveWindowArgs
            ))
            .unwrap(),
            writes: true,
        },
        ToolDefinition {
            name: "computer.act_checked",
            description: "Preferred model action: select one exact element ID from a <=120s observed frame, claim a lease, then dispatch once per request_id. The server checks the original window/document context and exact element against fresh native state immediately before single durable dispatch. No pixel/name locator, implicit focus, or replay; unknown/changed context fails closed. For click only, explicit JSON value {delivery:background_skylight_experimental} needs pointer_click.background_skylight_experimental implemented+available; one pointer down/up pair without AX action, activation, auth, primer or fallback. Plain action values still require the original element's advertised action. Typed drag paths use the selected element as origin; inspect pointer_drag capability, keep the exact frame, and verify independently. Release; Text/KeyChord still require exact focused UIA.",
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
            description: "Discover windows in the allocated desktop: at most 8 windows / 12 KiB structured data per page. On macOS each page also reports current displays with ColorSync UUIDs when available; numeric display ids and coordinates can change. Prefer an exact pid or title_contains filter. Use next_cursor alone for the same cached result; complete=false is not absence. Read-only, no focus or VM wakeup.",
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
    ]);
    tools
}
pub fn validate(tool: &str, args: &Value) -> Result<(), serde_json::Error> {
    match tool {
        "browser.snapshot" => {
            let a: super::browser::SnapshotArgs = serde_json::from_value(args.clone())?;
            a.validate().map_err(serde::de::Error::custom)
        }
        "browser.session" => {
            let a: super::browser::SessionArgs = serde_json::from_value(args.clone())?;
            a.validate()
                .map_err(<serde_json::Error as serde::de::Error>::custom)
        }
        "computer.recover_input" => {
            let _: RecoverInputArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.targets" => {
            let _: TargetsArgs = serde_json::from_value(args.clone())?;
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
        "computer.live" => {
            let _: LiveArgs = serde_json::from_value(args.clone())?;
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
        "computer.set_text" => {
            let _: SetTextArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.focus_element" => {
            let _: FocusElementArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.select_window_local" => {
            let _: WindowLocalArgs = serde_json::from_value(args.clone())?;
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
        "computer.bind_window" | "computer.attach_window" => {
            let _: BindArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.detach_window" => {
            let _: DetachWindowArgs = serde_json::from_value(args.clone())?;
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
        "computer.focus_window" => {
            let _: FocusWindowArgs = serde_json::from_value(args.clone())?;
            Ok(())
        }
        "computer.hotkey" => {
            let _: HotkeyArgs = serde_json::from_value(args.clone())?;
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
            "computer.targets"
                | "computer.operation"
                | "computer.cancel"
                | "browser.session"
                | "browser.snapshot"
        )
        && !cached_read(tool)
}

#[cfg(test)]
mod foreground_contract_tests {
    fn focus_request() -> serde_json::Value {
        serde_json::json!({
            "target": "fixture-target",
            "lease": "fixture-lease",
            "reference": "fixture-window-reference",
            "request_id": "focus-validator-fixture",
            "expected_foreground": {
                "pid": 7,
                "window": "cgwindow:77",
                "process_start": "fixture-process-start",
                "native_epoch": "fixture-native-epoch"
            }
        })
    }

    #[test]
    fn focus_validator_accepts_complete_request_and_keeps_required_fields() {
        let valid = focus_request();
        super::validate("computer.focus_window", &valid).unwrap();
        for field in [
            "target",
            "lease",
            "reference",
            "request_id",
            "expected_foreground",
        ] {
            let mut incomplete = valid.clone();
            incomplete.as_object_mut().unwrap().remove(field);
            assert!(
                super::validate("computer.focus_window", &incomplete).is_err(),
                "{field}"
            );
        }
        let mut wrong_type = valid;
        wrong_type["expected_foreground"]["pid"] = serde_json::json!("not-a-pid");
        assert!(super::validate("computer.focus_window", &wrong_type).is_err());
        assert_eq!(
            super::validate("computer.not_a_real_tool", &serde_json::json!({}))
                .unwrap_err()
                .to_string(),
            "unknown capability"
        );
    }

    #[test]
    fn focus_validator_catalog_covers_every_advertised_tool() {
        for definition in super::definitions() {
            // Missing required values can fail schema validation; an advertised
            // name must not fall through to the unknown-capability branch.
            if let Err(error) = super::validate(definition.name, &serde_json::json!({})) {
                assert_ne!(
                    error.to_string(),
                    "unknown capability",
                    "{}",
                    definition.name
                );
            }
        }
    }

    #[test]
    fn foreground_focus_catalog_has_explicit_window_ref_and_before_tuple() {
        let defs = super::definitions();
        let tool = defs.iter().find(|d| d.name == "computer.focus_window");
        assert!(tool.is_some(), "explicit macOS foreground route missing");
        let tool = tool.unwrap();
        assert!(tool.writes);
        for field in [
            "target",
            "reference",
            "lease",
            "request_id",
            "expected_foreground",
        ] {
            assert!(
                tool.input_schema["properties"].get(field).is_some(),
                "{field}"
            );
        }
    }
}

#[cfg(test)]
mod semantic_focus_wire_tests {
    #[test]
    fn semantic_focus_device_catalog_requires_internal_lease_and_rejects_options() {
        let valid = serde_json::json!({"target":"t", "frame":"f", "element":"e", "request_id":"once", "lease":"l"});
        assert!(super::validate("computer.focus_element", &valid).is_ok());
        assert!(super::executable("computer.focus_element"));
        let defs = super::definitions();
        let rows: Vec<_> = defs
            .iter()
            .filter(|d| d.name == "computer.focus_element")
            .collect();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].writes);
        assert_eq!(
            rows[0].input_schema["properties"]
                .as_object()
                .unwrap()
                .len(),
            5
        );
        for field in ["target", "frame", "element", "request_id", "lease"] {
            let mut incomplete = valid.clone();
            incomplete.as_object_mut().unwrap().remove(field);
            assert!(
                super::validate("computer.focus_element", &incomplete).is_err(),
                "{field}"
            );
        }
        for field in ["value", "options", "fallback", "include_timings", "intent"] {
            let mut extra = valid.clone();
            extra[field] = serde_json::json!(true);
            assert!(
                super::validate("computer.focus_element", &extra).is_err(),
                "{field}"
            );
        }
    }
}

#[cfg(test)]
mod window_local_wire_tests {
    #[test]
    fn device_window_selection_requires_internal_lease_and_exact_kind() {
        let valid = serde_json::json!({"target":"t","frame":"f","kind":"main","request_id":"once","lease":"l"});
        assert!(super::validate("computer.select_window_local", &valid).is_ok());
        assert!(super::executable("computer.select_window_local"));
        let defs = super::definitions();
        let rows: Vec<_> = defs
            .iter()
            .filter(|d| d.name == "computer.select_window_local")
            .collect();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].writes);
        assert_eq!(
            rows[0].input_schema["properties"]
                .as_object()
                .unwrap()
                .len(),
            5
        );
        for f in ["target", "frame", "kind", "request_id", "lease"] {
            let mut v = valid.clone();
            v.as_object_mut().unwrap().remove(f);
            assert!(
                super::validate("computer.select_window_local", &v).is_err(),
                "{f}"
            );
        }
        for f in [
            "element",
            "value",
            "options",
            "fallback",
            "include_timings",
            "intent",
            "expected_foreground",
        ] {
            let mut v = valid.clone();
            v[f] = serde_json::json!(true);
            assert!(
                super::validate("computer.select_window_local", &v).is_err(),
                "{f}"
            );
        }
        for kind in [
            serde_json::json!("focused"),
            serde_json::Value::Null,
            serde_json::json!(true),
        ] {
            let mut v = valid.clone();
            v["kind"] = kind;
            assert!(super::validate("computer.select_window_local", &v).is_err());
        }
    }
}

#[cfg(test)]
mod explicit_set_text_wire_tests {
    #[test]
    fn explicit_set_text_native_catalog_requires_six_fields_and_advertises_write() {
        let valid = serde_json::json!({"target":"t","frame":"f","element":"e","request_id":"once","text":"🙂","lease":"l"});
        super::validate("computer.set_text", &valid).unwrap();
        assert!(super::executable("computer.set_text"));
        let defs = super::definitions();
        let d = defs.iter().find(|d| d.name == "computer.set_text").unwrap();
        assert!(d.writes);
        assert_eq!(d.input_schema["properties"].as_object().unwrap().len(), 6);
        for field in ["target", "frame", "element", "request_id", "text", "lease"] {
            let mut bad = valid.clone();
            bad.as_object_mut().unwrap().remove(field);
            assert!(super::validate("computer.set_text", &bad).is_err());
        }
        for field in [
            "append",
            "options",
            "fallback",
            "intent",
            "value",
            "include_timings",
        ] {
            let mut bad = valid.clone();
            bad[field] = true.into();
            assert!(super::validate("computer.set_text", &bad).is_err());
        }
    }
}
