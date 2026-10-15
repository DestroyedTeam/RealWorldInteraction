//! Five-field explicit whole-value replacement; lease is internal.
use super::*;
use rwi_core::set_text::SetTextRequest;
pub(super) fn set_text_tool() -> Tool {
    let mut schema = serde_json::to_value(schemars::schema_for!(SetTextRequest)).unwrap();
    schema_numbers(&mut schema);
    Tool::new("computer.set_text","Explicitly replace the ENTIRE current writable plain, nonsecure AXValue in one exact macOS AXTextField/AXTextArea. Five fields target/frame/element/request_id/text. Plain UTF-8 0..4096 bytes (empty clears), one native setter; internal lease and fresh object/window/provider/context checks. Never append/caret/selection, type_text fallback, focus/activation, keyboard/composition, clipboard or retry. Full AXValue equality is separate observed_ax_replacement, never DOM/business goal or keyboard authority. Secure/unknown/specialized/read-only/unsupported targets refuse. Snapshot and independently read application/DOM afterward.",Arc::new(schema.as_object().unwrap().clone()))
        .with_title("Replace entire plain text value")
        .with_annotations(ToolAnnotations::new().read_only(false).destructive(true))
}
pub(super) async fn set_text<F, Fut>(args: SetTextRequest, call: F) -> DeviceResult
where
    F: FnMut(&'static str, Value, bool) -> Fut,
    Fut: Future<Output = DeviceResult>,
{
    if let Err(error) = args.validate() {
        return DeviceResult::error(&error.code, &error.next_step);
    }
    leased(
        &args.target,
        &args.request_id,
        "computer.set_text",
        json!({"frame":args.frame,"element":args.element,"text":args.text}),
        call,
    )
    .await
}
pub(super) fn compact_set_text_reply(data: &Value, details_ref: Option<String>) -> Value {
    let mut result = compact_focus_reply(data, details_ref);
    if let Some(value) = data.get("value_readback") {
        result["value_readback"] = value.clone();
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    fn request() -> SetTextRequest {
        SetTextRequest {
            target: "t".into(),
            frame: "f".into(),
            element: "e".into(),
            request_id: "once".into(),
            text: "🙂\n 中文 ".into(),
        }
    }
    #[test]
    fn explicit_set_text_model_schema_five_fields_no_hidden_lease_or_options() {
        let tool = set_text_tool();
        assert_eq!(
            tool.annotations.as_ref().unwrap().destructive_hint,
            Some(true)
        );
        assert_eq!(
            tool.input_schema["properties"].as_object().unwrap().len(),
            5
        );
        assert_eq!(tool.input_schema["additionalProperties"], false);
        let mut v = serde_json::to_value(request()).unwrap();
        assert!(!convenience_timing_option("computer.set_text", &mut v).unwrap());
        assert!(serde_json::from_value::<SetTextRequest>(v.clone()).is_ok());
        v["include_timings"] = true.into();
        assert!(serde_json::from_value::<SetTextRequest>(v).is_err());
    }
    #[tokio::test]
    async fn explicit_set_text_one_mutation_plaintext_no_retry_release_even_on_refusal() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let log = calls.clone();
        let output = set_text(request(), move |tool, args, cleanup| {
            log.lock().unwrap().push((tool, args, cleanup));
            async move {
                match tool {
                    "computer.claim" => DeviceResult::value(json!({"lease":"internal"})),
                    "computer.set_text" => {
                        DeviceResult::error("plain_string_value_required", "No setter.")
                    }
                    "computer.release" => DeviceResult::value(json!({"released":true})),
                    _ => panic!("unexpected fallback"),
                }
            }
        })
        .await;
        assert!(output.is_error);
        let calls = calls.lock().unwrap();
        assert_eq!(
            calls.iter().map(|c| c.0).collect::<Vec<_>>(),
            vec!["computer.claim", "computer.set_text", "computer.release"]
        );
        assert_eq!(calls[1].1["text"], request().text);
        assert_eq!(calls[1].1["lease"], "internal");
        assert!(calls[2].2);
    }
}
