//! Bounded, read-only diagnostic journal tail; never an execution or replay source.
use serde_json::Value;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
const MAX_BYTES: u64 = 65536;
const MAX_ROWS: usize = 20;
/// Latest complete valid JSONL records, oldest first. Partial/corrupt records are
/// omitted. The byte/count bounds apply even if a single record is oversized.
pub fn recent(path: &Path, limit: usize) -> Vec<Value> {
    let limit = limit.min(MAX_ROWS);
    if limit == 0 {
        return Vec::new();
    }
    let Ok(mut file) = File::open(path) else {
        return Vec::new();
    };
    let Ok(metadata) = file.metadata() else {
        return Vec::new();
    };
    if !metadata.is_file() {
        return Vec::new();
    }
    let start = metadata.len().saturating_sub(MAX_BYTES);
    // One preceding byte identifies an exact line boundary without parsing a
    // truncated UTF-8 prefix or treating a valid suffix as a complete record.
    let offset = start.saturating_sub(1);
    if file.seek(SeekFrom::Start(offset)).is_err() {
        return Vec::new();
    }
    let mut bytes = Vec::new();
    if file
        .take(metadata.len() - offset)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Vec::new();
    }
    let bytes = if start == 0 {
        bytes.as_slice()
    } else {
        let Some(newline) = bytes.iter().position(|b| *b == b'\n') else {
            return Vec::new();
        };
        &bytes[newline + 1..]
    };
    // A writer can be observed between bytes; only newline-terminated entries
    // are complete. Parse each line independently so one bad line is local.
    let Some(end) = bytes.iter().rposition(|b| *b == b'\n') else {
        return Vec::new();
    };
    let mut rows = bytes[..end]
        .split(|b| *b == b'\n')
        .rev()
        .filter_map(|line| serde_json::from_slice::<Value>(line).ok())
        .take(limit)
        .collect::<Vec<_>>();
    rows.reverse();
    rows
}
