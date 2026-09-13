//! Fresh exact-element revalidation without enumerating unrelated controls.
//! Locators never change: a focused/hit ancestor must equal the original ID.
use super::*;

fn element_id(w: HWND, pid: u32, epoch: &str, runtime: &[i32]) -> String {
    digest(format!("{}:{pid}:{epoch}:{runtime:?}", window_id(w)).as_bytes())
}

fn at_point(
    client: &Client,
    root: &IUIAutomationElement,
    w: HWND,
    pid: u32,
    epoch: &str,
    target: &rcu_core::Resolved,
    start: Instant,
) -> Result<IUIAutomationElement> {
    let mut e = {
        let _call = native_call("uia_target_from_point")?;
        api("uia_pointer_unavailable", unsafe {
            client.automation.ElementFromPointBuildCache(
                POINT {
                    x: target.point[0].round() as i32,
                    y: target.point[1].round() as i32,
                },
                &client.cache,
            )
        })?
    };
    let mut proof = crate::uia_ancestry::PointerAncestry::new(true);
    let mut selected = None;
    for _ in 0..32 {
        checkpoint("uia_target_ancestry")?;
        if start.elapsed() > Duration::from_millis(1500) {
            break;
        }
        let _call = native_call("uia_target_ancestry_native")?;
        let is_root = api("uia_ancestry_compare", unsafe {
            client.automation.CompareElements(root, &e)
        })?
        .as_bool();
        let exact = proof.needs_target()
            && target.scope.element.as_deref()
                == Some(element_id(w, pid, epoch, &runtime_id(&e)?).as_str());
        if exact {
            selected = Some(e.clone());
        }
        if proof.visit(is_root, exact)? {
            return selected.ok_or_else(|| {
                fault(
                    "uia_target_changed",
                    "Exact original target was not found; no substitute or replay.",
                )
            });
        }
        match client.related(&e, 2)? {
            Some(parent) => e = parent,
            None => {
                return Err(fault(
                    "uia_wrong_root",
                    "Target hit tree does not reach the selected native root.",
                ));
            }
        }
    }
    Err(fault(
        "uia_ancestry_incomplete",
        "Exact-target ancestry exceeded 32 levels / 1500 ms; no input.",
    ))
}

pub(crate) fn read(w: HWND, pid: u32, epoch: &str, target: &rcu_core::Resolved) -> Result<Walk> {
    let wanted = target.scope.element.as_deref().ok_or_else(|| {
        fault(
            "exact_element_required",
            "Targeted revalidation requires the original semantic ID.",
        )
    })?;
    let start = Instant::now();
    let client = Client::new()?;
    let root = client.root(w)?;
    let root_runtime_id = runtime_id(&root)?;
    let modal =
        unsafe { root.GetCurrentPatternAs::<IUIAutomationWindowPattern>(UIA_WindowPatternId) }
            .ok()
            .and_then(|p| unsafe { p.CurrentIsModal() }.ok())
            .map(|b| b.as_bool());
    let (focused, focus_unavailable) = match focus::read(&client, &root) {
        Ok(sample) => (Some(sample), None),
        Err(e) if e.code == "operation_cancelled" || e.code == "operation_deadline" => {
            return Err(e);
        }
        Err(e) => (None, Some(e.code)),
    };
    let exact_focus = focused
        .as_ref()
        .filter(|f| element_id(w, pid, epoch, &f.runtime) == wanted);
    let e = if let Some(focus) = exact_focus {
        focus.element.clone()
    } else {
        at_point(&client, &root, w, pid, epoch, target, start)?
    };
    // Ancestry can block/pump provider work. Never read stale cached properties
    // from before that proof, or use a plain ElementFromPoint as a full cache.
    let e = {
        let _call = native_call("uia_target_refresh_cache")?;
        api("uia_target_refresh_failed", unsafe {
            e.BuildUpdatedCache(&client.cache)
        })?
    };
    if element_id(w, pid, epoch, &runtime_id(&e)?) != wanted {
        return Err(fault(
            "uia_target_changed",
            "Refreshed element no longer has the original runtime identity.",
        ));
    }
    client.under_root(&root, &e)?;
    let mut reader =
        observation::Reader::new(&client, w, pid, epoch, start, focused.map(|f| f.runtime));
    reader.append(&e, None)?;
    checkpoint("uia_target_read_returned")?;
    Ok(Walk {
        root_runtime_id,
        elements: reader.elements,
        details: reader.details,
        modal,
        complete: false,
        truncation: Some(
            "one exact target revalidated; unrelated descendants not requested".into(),
        ),
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.,
        focus_unavailable,
        document_state: None,
        document_context_unavailable: Some(
            "native-element policy; not a whole-document proof".into(),
        ),
    })
}
