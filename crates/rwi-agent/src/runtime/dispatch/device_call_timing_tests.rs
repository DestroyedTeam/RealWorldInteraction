use super::*;
use std::time::Duration;

#[test]
fn device_call_timing_absent_when_call_not_entered() {
    let mut result = DeviceResult::error("refused", "Do not replay.");
    let original = result.data.clone();
    append_device_call_timing(&mut result, "executor_call", None);
    assert_eq!(result.data, original);
    assert!(result.is_error);
}
#[test]
fn device_call_timing_preserves_error_effect_and_images() {
    let mut result = DeviceResult::error("uncertain", "Inspect, do not replay.");
    result.data["dispatch"] = json!("uncertain");
    result.images.push(rwi_core::device::DeviceImage {
        mime: "image/png".into(),
        base64: "original-bytes".into(),
    });
    let original = result.data.clone();
    append_device_call_timing(
        &mut result,
        "executor_call",
        Some(Duration::from_micros(1250)),
    );
    assert_eq!(
        result.data["device_call_wall"],
        json!({"scope":"executor_call","elapsed_us":1250,"cpu_measured":false})
    );
    result
        .data
        .as_object_mut()
        .unwrap()
        .remove("device_call_wall");
    assert_eq!(result.data, original);
    assert!(result.is_error);
    assert_eq!(result.images[0].base64, "original-bytes");
}
#[test]
fn device_call_timing_retains_existing_sample() {
    let mut result = DeviceResult::value(json!({"device_call_wall":{"future":"original"}}));
    let original = result.data.clone();
    append_device_call_timing(&mut result, "executor_call", Some(Duration::from_secs(3)));
    assert_eq!(result.data, original);
}
#[test]
fn device_call_timing_does_not_rewrite_nonobject_payload() {
    let mut result = DeviceResult::value(json!([1, 2, 3]));
    append_device_call_timing(&mut result, "executor_call", Some(Duration::ZERO));
    assert_eq!(result.data, json!([1, 2, 3]));
    assert!(!result.is_error);
}
