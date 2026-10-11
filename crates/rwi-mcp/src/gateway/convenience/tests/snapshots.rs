use super::*;

#[test]
fn fenced_snapshot_is_never_advertised_as_mutation_ready() {
    assert_eq!(
        snapshot_mutation_ready(
            &json!({"input_domain_fenced":true,"scene":{"mutation_ready":true}})
        ),
        false
    );
    assert_eq!(
        snapshot_mutation_ready(
            &json!({"input_domain_fenced":false,"scene":{"mutation_ready":true}})
        ),
        true
    );
    assert!(snapshot_mutation_ready(&json!({})).is_null());
}

#[test]
fn visual_only_snapshot_hides_even_a_legacy_agent_view_id() {
    let legacy_image = json!({"view_id":"old-agent-view"});
    let visual_only = json!({"binding":null,"pixel_to_desktop":null});
    assert_eq!(
        snapshot_input_view(&visual_only, &legacy_image),
        (Value::Null, false)
    );
    let semantic_only = json!({"binding":{"scope":{}},"pixel_to_desktop":null});
    assert_eq!(
        snapshot_input_view(&semantic_only, &legacy_image),
        (Value::Null, false)
    );
    let grounded = json!({"binding":{"scope":{}},"pixel_to_desktop":[1,0,0,1,0,0]});
    assert_eq!(
        snapshot_input_view(&grounded, &legacy_image),
        (json!("old-agent-view"), true)
    );
    assert_eq!(
        snapshot_input_view(
            &grounded,
            &json!({"view_id":null,"input_reference_available":false})
        ),
        (Value::Null, false)
    );
}

#[test]
fn snapshot_budget_keeps_anonymous_close_ahead_of_hidden_row_noise() {
    let rows = vec![
        json!({"id":"offscreen-row","role":"AXRow","name":"","actions":["AXShowDefaultUI"],"point":[10,10000],"visible":false,"occluded":true}),
        json!({"id":"close","role":"AXButton","name":"","actions":["AXPress"],"point":[17,19],"enabled":true}),
    ];
    let (out, total) = compact_controls(&rows, 1);
    assert_eq!(total, 2);
    assert_eq!(out[0]["id"], "close");
    assert_eq!(out[0]["point"], json!([17, 19]));
    assert_eq!(out[0]["actions"], json!(["AXPress"]));
}
#[test]
fn snapshot_removes_non_native_action_noise_without_inventing_state() {
    let rows = vec![
        json!({"id":"x","name":"","description":"Undo","actions":["AXPress","Name:Move next\nTarget:0x0\nSelector:(null)"],"visible":true,"enabled":false,"occluded":false}),
        json!({"id":"noise","actions":["Name:Move next"]}),
    ];
    let (out, total) = compact_controls(&rows, 64);
    assert_eq!(total, 1);
    assert_eq!(out[0]["name"], "Undo");
    assert_eq!(out[0]["actions"], json!(["AXPress"]));
    assert_eq!(out[0]["enabled"], false);
    assert!(out[0].get("visible").is_none());
    assert!(out[0].get("selected").is_none());
}
