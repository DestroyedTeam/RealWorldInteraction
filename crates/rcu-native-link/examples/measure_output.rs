//! Explicit offline replay of saved RPC responses, never an OS/MCP input bridge.
use rcu_native_link::presentation::Presenter;
use serde_json::{Value, json};
use std::{io, path::PathBuf, time::Instant};
fn text_bytes(wire: &Value) -> usize {
    wire["result"]["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["text"].as_str())
        .map(str::len)
        .sum()
}
fn main() -> io::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let output = PathBuf::from(
        args.next()
            .ok_or_else(|| io::Error::other("output directory required"))?,
    );
    let presenter = Presenter::open(&output)?;
    let mut rows = Vec::new();
    for (id, file) in args.enumerate() {
        let source: Value = serde_json::from_slice(&std::fs::read(&file)?)?;
        let before = if source.get("jsonrpc").is_some() {
            source
        } else {
            json!({"jsonrpc":"2.0","id":id,"result":source})
        };
        let raw = serde_json::to_vec(&before)?;
        let start = Instant::now();
        let after_raw = presenter.render(raw.clone())?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.;
        let after: Value = serde_json::from_slice(&after_raw)?;
        let name = format!("compact-{id:03}.json");
        std::fs::write(output.join(&name), &after_raw)?;
        rows.push(json!({"source":PathBuf::from(file),"before_wire_bytes":raw.len(),"after_wire_bytes":after_raw.len(),"before_text_bytes":text_bytes(&before),"after_text_bytes":text_bytes(&after),"projection_and_durable_archive_ms":elapsed,"result":name,"native_runtime_executed":false}));
    }
    println!("{}", serde_json::to_string_pretty(&rows)?);
    Ok(())
}
