//! Explain refusal dimensions without values, pixels or renewed authority.
use crate::{Resolved, Scene};
use serde_json::{Value, json};
pub fn grounding_diagnostic(before: &Scene, current: &Scene, reference: &Resolved) -> Value {
    let mut data = match (&before.binding, &current.binding) {
        (Some(expected), Some(now)) => json!({"scope_changed":expected.scope!=now.scope,
            "focus_changed":expected.focus_window!=now.focus_window,"layout_changed":expected.layout!=now.layout,
            "native_epoch_changed":expected.native_epoch!=now.native_epoch,
            "mutation_context_changed":expected.mutation_context!=now.mutation_context,"content_changed":expected.content!=now.content}),
        _ => json!({"binding_missing":true}),
    };
    data["reference_kind"] = json!(if reference.scope.element.is_some() {
        "element"
    } else {
        "pixel"
    });
    if reference.scope.element.is_none() {
        data["local_pixel_anchor_available"] = json!(reference.pixel_anchor.is_some());
        data["red_marker_anchor_available"] = json!(reference.red_marker_anchor.is_some());
        data["pixel_content_policy"] = json!(if reference.red_marker_anchor.is_some() {
            "explicit_red_marker_cluster"
        } else if reference.pixel_anchor.is_some() {
            "bounded_local_patch"
        } else {
            "full_frame_equality"
        });
        // Availability is known; a lost construction reason is not guessed
        // from a later frame or classified as texture/permission failure.
        if reference.pixel_anchor.is_none() && reference.red_marker_anchor.is_none() {
            data["local_anchor_creation_reason"] = json!("not_recorded");
        }
    }
    data["native_input_dispatched"] = json!(false);
    data["input_authority"] = json!(false);
    data["mutation_ready_changed"] = json!(before.mutation_ready != current.mutation_ready);
    data["blocked_changed"] = json!(before.blocked != current.blocked);
    if let Some(id) = &reference.scope.element {
        let expected = before.elements.iter().find(|e| &e.id == id);
        let now = current.elements.iter().find(|e| &e.id == id);
        let mut element = json!({"observed_found":expected.is_some(),"current_found":now.is_some(),
            "point_matches_reference":now.map(|e|e.point==reference.point),
            "revision_matches_reference":now.map(|e|Some(e.action_revision(reference.native_action.as_deref()))==reference.element_revision),
            "native_action_requested":reference.native_action.is_some(),
            "native_action_supported":reference.native_action.as_ref().and_then(|action|now.map(|e|e.actions.contains(action))),
            "pointer_visibility_ready":now.map(|e|e.visible==Some(true)&&e.occluded==Some(false))});
        if let (Some(a), Some(b)) = (expected, now) {
            // Field names and equality flags only. Never export the actual text,
            // selected values, point, revision hash or diagnostic-as-action handle.
            element["changed"] = json!({"point":a.point!=b.point,"name":a.name!=b.name,"role":a.role!=b.role,
                "enabled":a.enabled!=b.enabled,"actions":a.actions!=b.actions,"selected":a.selected!=b.selected,
                "value":a.value!=b.value,"value_truncated":a.value_truncated!=b.value_truncated,
                "state_revision":a.state_revision!=b.state_revision,"description":a.description!=b.description,
                "visible":a.visible!=b.visible,"occluded":a.occluded!=b.occluded});
        }
        data["element"] = element;
    }
    data
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Backend, Engine, Locator, fixture::Fixture, grounding_matches};
    #[test]
    fn element_refusal_is_explained_even_when_all_binding_dimensions_match() {
        let dir = tempfile::tempdir().unwrap();
        let fixture = Fixture::default();
        let mut e = Engine::new(dir.path().into()).unwrap();
        e.register("fixture".into(), Box::new(fixture.clone()))
            .unwrap();
        let shot = e.observe("fixture").unwrap();
        let r = e
            .resolve(
                "fixture",
                &shot.frame,
                Locator::Element { id: "Input".into() },
            )
            .unwrap();
        let before = fixture.clone().observe().unwrap();
        let mut now = before.clone();
        now.elements
            .iter_mut()
            .find(|e| e.id == "Input")
            .unwrap()
            .state_revision = Some("private-value-marker".into());
        assert!(!grounding_matches(
            before.binding.as_ref().unwrap(),
            &now,
            &r
        ));
        let d = grounding_diagnostic(&before, &now, &r);
        assert_eq!(d["scope_changed"], false);
        assert_eq!(d["element"]["changed"]["state_revision"], true);
        assert_eq!(d["element"]["revision_matches_reference"], false);
        assert!(
            !serde_json::to_string(&d)
                .unwrap()
                .contains("private-value-marker")
        );
        now.elements.retain(|e| e.id != "Input");
        assert_eq!(
            grounding_diagnostic(&before, &now, &r)["element"]["current_found"],
            false
        );
        assert!(!grounding_matches(
            before.binding.as_ref().unwrap(),
            &now,
            &r
        ));
    }
}
