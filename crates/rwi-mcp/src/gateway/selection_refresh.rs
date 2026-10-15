//! One bounded, identity-preserving preparation after a clean catalog rejection.
//! Never applies to input, bind_window, context refresh or uncertain attachment.
use super::*;
use std::future::Future;

const MAX_PAGES: usize = 8;
const MAX_ROWS: usize = 2048;

impl Gateway {
    pub(super) async fn attach_with_fresh_catalog(
        &self,
        args: Value,
        context: Option<RequestContext<RoleServer>>,
    ) -> DeviceResult {
        let initial = self
            .invoke_device("computer.attach_window", args.clone(), context.clone())
            .await;
        let (mut result, details) = refresh_expired(args, initial, |tool, args| {
            self.invoke_device(tool, args, context.clone())
        })
        .await;
        if let Some(details) = details {
            let mut summary = details["summary"].clone();
            match self.presentation.lock() {
                Ok(mut cache) => {
                    let reference = cache.retain(
                        "computer.attach_window",
                        json!({"result": result.data, "selection_preparation": details}),
                    );
                    summary["details_ref"] = json!(reference);
                }
                Err(_) => summary["details"] = details,
            }
            result.data["selection_preparation"] = summary;
        }
        result
    }
}

fn clean(result: &DeviceResult) -> bool {
    let d = &result.data;
    d["native_dispatch_entered"] == false
        && d["input_domain_fenced"] == false
        && d["operation"]["terminal"] == true
        && d["operation"]["resources_released"] == true
        && d["operation"]["native_call_pending"] == false
        && d["operation"]["resource_release_uncertain"] == false
        && d["operation"]["cancel_requested"] == false
}

fn reference(result: &DeviceResult) -> Option<device::OperationRef> {
    serde_json::from_value::<device::OperationRef>(result.data["operation_ref"].clone())
        .ok()
        .filter(device::OperationRef::valid_shape)
}

fn same_context(a: &device::OperationRef, b: &device::OperationRef) -> bool {
    a.device_id == b.device_id
        && a.agent_instance == b.agent_instance
        && a.context_generation == b.context_generation
}

fn bounded_string(value: &Value, max: usize) -> Option<&str> {
    value.as_str().filter(|s| !s.is_empty() && s.len() <= max)
}

fn finished(
    result: DeviceResult,
    initial: &DeviceResult,
    pages: Vec<Value>,
    reason: &str,
    attached: bool,
) -> (DeviceResult, Option<Value>) {
    let summary = json!({
        "reason": reason, "catalog_pages": pages.len(),
        "fresh_attachment_attempted": attached,
        "selection_substituted": false, "requested_input_replayed": false
    });
    let details = json!({"summary":summary, "initial_error":initial.data,
        "initial_is_error":initial.is_error, "discovery_pages":pages});
    (result, Some(details))
}

fn refused(
    initial: &DeviceResult,
    pages: Vec<Value>,
    reason: &str,
) -> (DeviceResult, Option<Value>) {
    finished(
        DeviceResult::error(
            "selection_refresh_unverified",
            "No fresh attachment sent. Inspect selection_preparation and discover explicitly; do not substitute a new window or epoch.",
        ),
        initial,
        pages,
        reason,
        false,
    )
}

async fn refresh_expired<F, Fut>(
    args: Value,
    initial: DeviceResult,
    mut call: F,
) -> (DeviceResult, Option<Value>)
where
    F: FnMut(&'static str, Value) -> Fut,
    Fut: Future<Output = DeviceResult>,
{
    let parsed = serde_json::from_value::<device::api::BindArgs>(args.clone()).ok();
    let expected = reference(&initial);
    if !initial.is_error
        || initial.data["code"] != "catalog_expired"
        || !clean(&initial)
        || initial.data.get("dispatch").is_some()
        || initial.data.get("receipt").is_some()
        || initial.data.get("target").is_some()
        || !initial.images.is_empty()
        || parsed.is_none()
        || expected.is_none()
    {
        return (initial, None);
    }
    let parsed = parsed.unwrap();
    let selection = &parsed.selection;
    if !matches!(selection.relation, rwi_core::WindowRelation::Initial)
        || selection.pid == 0
        || selection.window.is_empty()
        || selection.native_epoch.is_empty()
    {
        return (initial, None);
    }
    let expected = expected.unwrap();
    let mut query = json!({"target":parsed.target,"pid":selection.pid});
    let mut pages = Vec::new();
    let mut catalog = None;
    let mut total = None;
    let mut cursors = BTreeSet::new();
    let mut windows = BTreeSet::new();
    let mut found = false;
    for _ in 0..MAX_PAGES {
        let page = call("computer.discover", query).await;
        pages.push(page.data.clone());
        if page.is_error {
            return finished(page, &initial, pages, "discovery_error", false);
        }
        if !clean(&page)
            || !page.images.is_empty()
            || page.data["activated"] != false
            || page.data["input_sent"] != false
            || page.data["enumeration_complete"] != true
            || reference(&page).is_none_or(|r| !same_context(&expected, &r))
            || page.data["filter"]["pid"].as_u64() != Some(u64::from(selection.pid))
            || page.data["filter"].get("title_contains") != Some(&Value::Null)
        {
            return refused(&initial, pages, "discovery_not_clean_complete_same_context");
        }
        let id = bounded_string(&page.data["catalog_id"], 128).map(str::to_owned);
        let count = page.data["matching_windows"]
            .as_u64()
            .filter(|n| *n <= MAX_ROWS as u64);
        if id.is_none() || count.is_none() {
            return refused(&initial, pages, "catalog_header_invalid");
        }
        if catalog.as_ref().is_some_and(|old| Some(old) != id.as_ref())
            || total.is_some_and(|old| Some(old) != count)
        {
            return refused(&initial, pages, "catalog_changed_between_pages");
        }
        catalog = id;
        total = count;
        let Some(rows) = page.data["windows"].as_array() else {
            return refused(&initial, pages, "rows_invalid");
        };
        for row in rows {
            let Some(window) = bounded_string(&row["window"], 512) else {
                return refused(&initial, pages, "window_identity_invalid");
            };
            let Some(epoch) = bounded_string(&row["native_epoch"], 512) else {
                return refused(&initial, pages, "epoch_invalid");
            };
            if row["pid"].as_u64() != Some(u64::from(selection.pid))
                || !windows.insert(window.to_owned())
                || windows.len() > MAX_ROWS
            {
                return refused(&initial, pages, "foreign_or_duplicate_window");
            }
            if window == selection.window {
                if epoch != selection.native_epoch {
                    return refused(&initial, pages, "selection_epoch_changed");
                }
                found = true;
            }
        }
        match page.data.get("next_cursor") {
            Some(Value::Null) => {
                if total != Some(windows.len() as u64) || !found {
                    return refused(&initial, pages, "selection_not_verified_in_bounded_catalog");
                }
                // The original args, including relation and correlation, are exact.
                // The native backend still revalidates authority and window identity.
                let attached = call("computer.attach_window", args).await;
                if !attached.data.is_object() {
                    let mut invalid = DeviceResult::error(
                        "attachment_result_unstructured",
                        "Fresh attachment result is unknown. Inspect retained details; do not retry.",
                    );
                    invalid.data["original_attachment_reply"] = attached.data;
                    return finished(invalid, &initial, pages, "attachment_reply_unknown", true);
                }
                return finished(attached, &initial, pages, "fresh_attach_returned", true);
            }
            Some(cursor) => {
                let Some(cursor) = bounded_string(cursor, 96) else {
                    return refused(&initial, pages, "cursor_invalid");
                };
                if rows.is_empty()
                    || total.is_some_and(|n| windows.len() as u64 >= n)
                    || !cursors.insert(cursor.to_owned())
                {
                    return refused(&initial, pages, "cursor_loop_or_count_mismatch");
                }
                query = json!({"target":parsed.target,"cursor":cursor});
            }
            None => return refused(&initial, pages, "cursor_unknown"),
        }
    }
    refused(&initial, pages, "page_budget_exhausted")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args() -> Value {
        json!({"target":"device:agent:context","selection":{"window":"window:7","pid":77,"native_epoch":"epoch-7","relation":"initial"},"correlation_id":"caller-correlation"})
    }
    fn facts() -> Value {
        json!({"native_dispatch_entered":false,"input_domain_fenced":false,
            "operation":{"terminal":true,"resources_released":true,"native_call_pending":false,"resource_release_uncertain":false,"cancel_requested":false},
            "operation_ref":{"ref_version":1,"device_id":"device","agent_instance":"agent","context_generation":"context","execution_nonce":"nonce"}})
    }
    fn expired() -> DeviceResult {
        let mut r = DeviceResult::error("catalog_expired", "Original rejection retained.");
        r.data
            .as_object_mut()
            .unwrap()
            .extend(facts().as_object().unwrap().clone());
        r
    }
    fn page() -> DeviceResult {
        let mut v = facts();
        v.as_object_mut().unwrap().extend(
            json!({"activated":false,"input_sent":false,
            "enumeration_complete":true,"filter":{"pid":77,"title_contains":null},
            "catalog_id":"catalog","matching_windows":1,"next_cursor":null,
            "windows":[{"window":"window:7","pid":77,"native_epoch":"epoch-7"}]})
            .as_object()
            .unwrap()
            .clone(),
        );
        DeviceResult::value(v)
    }
    async fn run(
        initial: DeviceResult,
        replies: Vec<DeviceResult>,
    ) -> (DeviceResult, Option<Value>, Vec<(&'static str, Value)>) {
        let mut replies = std::collections::VecDeque::from(replies);
        let mut calls = Vec::new();
        let (r, d) = refresh_expired(args(), initial, |tool, args| {
            calls.push((tool, args));
            std::future::ready(replies.pop_front().expect("unexpected extra call"))
        })
        .await;
        (r, d, calls)
    }
    #[tokio::test]
    async fn f77_selection_refresh_normal_success_and_other_errors_unchanged() {
        for initial in [
            DeviceResult::value(json!({"target":"child"})),
            DeviceResult::error("context_unknown", "Refresh explicitly."),
            DeviceResult::error("window_target_capacity", "Do not retry."),
        ] {
            let original = initial.data.clone();
            let flag = initial.is_error;
            let (r, d, c) = run(initial, vec![]).await;
            assert_eq!(r.data, original);
            assert_eq!(r.is_error, flag);
            assert!(d.is_none() && c.is_empty());
        }
    }
    #[tokio::test]
    async fn f77_selection_refresh_requires_every_clean_fact_not_truthiness() {
        for ptr in [
            "/native_dispatch_entered",
            "/input_domain_fenced",
            "/operation/terminal",
            "/operation/resources_released",
            "/operation/native_call_pending",
            "/operation/resource_release_uncertain",
            "/operation/cancel_requested",
        ] {
            for wrong in [Value::Null, json!("false"), json!(1)] {
                let mut e = expired();
                *e.data.pointer_mut(ptr).unwrap() = wrong;
                let (r, d, c) = run(e.clone(), vec![]).await;
                assert_eq!(r.data, e.data);
                assert!(d.is_none() && c.is_empty(), "{ptr}");
            }
        }
        for (ptr, wrong) in [
            ("/native_dispatch_entered", true),
            ("/input_domain_fenced", true),
            ("/operation/terminal", false),
            ("/operation/resources_released", false),
            ("/operation/native_call_pending", true),
            ("/operation/resource_release_uncertain", true),
            ("/operation/cancel_requested", true),
        ] {
            let mut e = expired();
            *e.data.pointer_mut(ptr).unwrap() = json!(wrong);
            let (_, d, c) = run(e, vec![]).await;
            assert!(d.is_none() && c.is_empty());
        }
    }
    #[tokio::test]
    async fn f77_selection_refresh_no_uncertain_receipt_or_existing_target() {
        for field in ["dispatch", "receipt", "target"] {
            let mut e = expired();
            e.data[field] = Value::Null;
            let (_, d, c) = run(e, vec![]).await;
            assert!(d.is_none() && c.is_empty());
        }
        let mut e = expired();
        e.data["operation_ref"] = Value::Null;
        let (_, d, c) = run(e, vec![]).await;
        assert!(d.is_none() && c.is_empty());
    }
    #[tokio::test]
    async fn f77_selection_refresh_exact_selection_args_once_and_retained_original() {
        let old = expired();
        let original = old.data.clone();
        let (r, d, c) = run(
            old,
            vec![page(), DeviceResult::value(json!({"target":"new-child"}))],
        )
        .await;
        assert!(!r.is_error);
        assert_eq!(
            c,
            vec![
                (
                    "computer.discover",
                    json!({"target":"device:agent:context","pid":77})
                ),
                ("computer.attach_window", args())
            ]
        );
        let d = d.unwrap();
        assert_eq!(d["initial_error"], original);
        assert_eq!(d["summary"]["fresh_attachment_attempted"], true);
        assert_eq!(d["summary"]["requested_input_replayed"], false);
    }
    #[tokio::test]
    async fn f77_selection_refresh_identity_or_context_change_never_substitutes() {
        for (ptr, wrong) in [
            ("/windows/0/native_epoch", json!("new-epoch")),
            ("/windows/0/window", json!("window:8")),
            ("/windows/0/pid", json!(78)),
            ("/operation_ref/device_id", json!("other")),
            ("/operation_ref/agent_instance", json!("other")),
            ("/operation_ref/context_generation", json!("other")),
        ] {
            let mut p = page();
            *p.data.pointer_mut(ptr).unwrap() = wrong;
            let (r, _, c) = run(expired(), vec![p]).await;
            assert!(r.is_error);
            assert_eq!(c.len(), 1, "{ptr}");
        }
    }
    #[tokio::test]
    async fn f77_selection_refresh_partial_or_effectful_catalog_not_eligible() {
        for (ptr, wrong) in [
            ("/enumeration_complete", json!(false)),
            ("/input_sent", json!(true)),
            ("/activated", json!(true)),
            ("/operation/resources_released", json!(false)),
            ("/operation/native_call_pending", json!(true)),
            ("/filter/pid", json!(78)),
            ("/filter/title_contains", json!("title")),
        ] {
            let mut p = page();
            *p.data.pointer_mut(ptr).unwrap() = wrong;
            let (r, _, c) = run(expired(), vec![p]).await;
            assert!(r.is_error);
            assert_eq!(c.len(), 1, "{ptr}");
        }
    }
    #[tokio::test]
    async fn f77_selection_refresh_malformed_unknown_and_duplicate_rows_refuse() {
        for case in 0..7 {
            let mut p = page();
            match case {
                0 => p.data["windows"] = Value::Null,
                1 => p.data["windows"][0]["pid"] = json!("77"),
                2 => p.data["windows"][0]["native_epoch"] = json!(""),
                3 => {
                    let duplicate = p.data["windows"][0].clone();
                    p.data["windows"].as_array_mut().unwrap().push(duplicate);
                }
                4 => p.data["matching_windows"] = json!(2),
                5 => {
                    p.data.as_object_mut().unwrap().remove("next_cursor");
                }
                _ => p.data["next_cursor"] = json!(false),
            };
            let (r, _, c) = run(expired(), vec![p]).await;
            assert!(r.is_error);
            assert_eq!(c.len(), 1, "case{case}");
        }
    }
    #[tokio::test]
    async fn f77_selection_refresh_pagination_preserves_catalog_and_cursor_only_query() {
        let mut a = page();
        a.data["matching_windows"] = json!(2);
        a.data["next_cursor"] = json!("opaque/1");
        let mut b = page();
        b.data["matching_windows"] = json!(2);
        b.data["windows"][0]["window"] = json!("window:8");
        let (r, _, c) = run(
            expired(),
            vec![a, b, DeviceResult::value(json!({"target":"child"}))],
        )
        .await;
        assert!(!r.is_error);
        assert_eq!(c.len(), 3);
        assert_eq!(
            c[1],
            (
                "computer.discover",
                json!({"target":"device:agent:context","cursor":"opaque/1"})
            )
        );
        assert_eq!(c[2], ("computer.attach_window", args()));
    }
    #[tokio::test]
    async fn f77_selection_refresh_changed_catalog_and_cursor_loop_stop() {
        for changed_catalog in [false, true] {
            let mut a = page();
            a.data["matching_windows"] = json!(3);
            a.data["next_cursor"] = json!("cursor");
            let mut b = a.clone();
            b.data["windows"][0]["window"] = json!("window:8");
            if changed_catalog {
                b.data["catalog_id"] = json!("other");
            }
            let (r, _, c) = run(expired(), vec![a, b]).await;
            assert!(r.is_error);
            assert_eq!(c.len(), 2);
        }
    }
    #[tokio::test]
    async fn f77_selection_refresh_page_budget_is_bounded_without_absence_claim() {
        let mut pages = Vec::new();
        for i in 0..MAX_PAGES {
            let mut p = page();
            p.data["matching_windows"] = json!(100);
            p.data["windows"][0]["window"] = json!(format!("window:{i}"));
            p.data["next_cursor"] = json!(format!("cursor/{i}"));
            pages.push(p);
        }
        let (r, d, c) = run(expired(), pages).await;
        assert!(r.is_error);
        assert_eq!(c.len(), MAX_PAGES);
        assert_eq!(d.unwrap()["summary"]["reason"], "page_budget_exhausted");
        assert!(r.data.get("absence_proven").is_none());
    }
    #[tokio::test]
    async fn f77_selection_refresh_discovery_failure_returned_with_original_retained() {
        let failed = DeviceResult::error("context_unknown", "No context retarget.");
        let (r, d, c) = run(expired(), vec![failed.clone()]).await;
        assert_eq!(r.data, failed.data);
        assert_eq!(c.len(), 1);
        assert_eq!(d.unwrap()["initial_error"]["code"], "catalog_expired");
    }
    #[tokio::test]
    async fn f77_selection_refresh_final_attach_error_never_retries() {
        let failed = expired();
        let (r, d, c) = run(expired(), vec![page(), failed.clone()]).await;
        assert_eq!(r.data, failed.data);
        assert_eq!(c.len(), 2);
        assert_eq!(d.unwrap()["summary"]["fresh_attachment_attempted"], true);
    }
    #[tokio::test]
    async fn f77_selection_refresh_unstructured_final_reply_is_unknown_not_success() {
        let (r, d, c) = run(expired(), vec![page(), DeviceResult::value(Value::Null)]).await;
        assert!(r.is_error);
        assert_eq!(r.data["code"], "attachment_result_unstructured");
        assert_eq!(r.data["original_attachment_reply"], Value::Null);
        assert_eq!(c.len(), 2);
        assert_eq!(d.unwrap()["summary"]["fresh_attachment_attempted"], true);
    }
    #[tokio::test]
    async fn f77_selection_refresh_non_initial_or_invalid_selection_is_untouched() {
        for (ptr, wrong) in [
            ("/selection/relation", json!("owned_popup")),
            ("/selection/pid", json!(0)),
            ("/selection/native_epoch", json!("")),
        ] {
            let mut a = args();
            *a.pointer_mut(ptr).unwrap() = wrong;
            let (r, d) = refresh_expired(a, expired(), |_, _| async { panic!("no call") }).await;
            assert_eq!(r.data["code"], "catalog_expired");
            assert!(d.is_none());
        }
    }
}
