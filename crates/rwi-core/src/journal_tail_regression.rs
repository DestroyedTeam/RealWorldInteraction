//! Offline regression: diagnostics only; no targets, native calls or input.
use super::*;
use serde_json::{Value, json};
fn journal(text: &str, limit: usize) -> Vec<Value> {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::new(dir.path().to_owned()).unwrap();
    fs::write(dir.path().join("operations.jsonl"), text).unwrap();
    engine.recent_operations(limit)
}
#[test]
fn journal_tail_regression_reads_recent_records_not_the_first_64k() {
    let text = (0..2000)
        .map(|i| {
            format!(
                "{}\n",
                json!({"ts_unix_ms":i,"label":"bounded diagnostic record"})
            )
        })
        .collect::<String>();
    assert!(text.len() > 65536);
    assert_eq!(
        journal(&text, 2)
            .iter()
            .map(|v| v["ts_unix_ms"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![1998, 1999]
    );
}
#[test]
fn journal_tail_regression_discards_partial_and_invalid_records_without_losing_valid_tail() {
    let text = format!(
        "{}\n{}\ninvalid\n{}\n{{\"partial\":",
        json!({"ts_unix_ms":1}),
        json!({"ts_unix_ms":2}),
        json!({"ts_unix_ms":3})
    );
    assert_eq!(
        journal(&text, 2),
        vec![json!({"ts_unix_ms":2}), json!({"ts_unix_ms":3})]
    );
}
#[test]
fn journal_tail_regression_utf8_cut_and_oversized_record_do_not_poison_tail() {
    let text = format!(
        "{}\n{}\n{}\n",
        json!({"text":"字".repeat(30000)}),
        json!({"ts_unix_ms":8}),
        json!({"ts_unix_ms":9})
    );
    assert_eq!(
        journal(&text, 2),
        vec![json!({"ts_unix_ms":8}), json!({"ts_unix_ms":9})]
    );
}
#[test]
fn journal_tail_regression_caps_count_and_zero_is_empty() {
    let text = (0..25)
        .map(|i| format!("{}\n", json!({"ts_unix_ms":i})))
        .collect::<String>();
    assert_eq!(journal(&text, 0).len(), 0);
    assert_eq!(journal(&text, usize::MAX).len(), 20);
}
