#[test]
fn timing_guidance_is_schema_scoped_and_keeps_no_replay() {
    let instructions = super::SERVER_INSTRUCTIONS;
    assert!(instructions.contains("Add include_timings only if exposed by the tool input schema"));
    assert!(!instructions.contains("include_timings=true adds"));
    assert!(instructions.contains("Never replay uncertain input"));
    assert!(instructions.contains("elapsed_ms is pre-cache"));
}
