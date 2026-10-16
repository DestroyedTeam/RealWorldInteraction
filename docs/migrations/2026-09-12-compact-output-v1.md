# Adapter compact output v1

This is a presentation-layer change, not a device wire migration. The native
agent still has 20 device capabilities and uses device wire v2. The MCP adapter
adds its own read-only `computer.result` tool, making its catalog 21 tools.
Metadata retrieval is served locally and never forwarded to a device.

## Changed default responses

Successful `computer.observe` and `computer.targets` receive a
`presentation.version = compact_v1` envelope. Errors, native operation references,
durable request identity, cancellation state and uncertainty are not rewritten.
Original image blocks are passed through, not recompressed.

- Targets are present once at the top level; duplicated device-state target lists
  are omitted. Per-device availability/errors remain visible.
- Observation retains scene identity/binding, coordinates, readiness, elements
  and focused-element IDs. Per-element native `control_type` and positive
  `keyboard_focus` evidence are brought alongside elements when available.
  Missing keyboard_focus is not an attested false value.
- Full duplicated UIA diagnostics, state-revision strings and selected repeated
  contract prose are omitted from the compact view, not from retained metadata.
  Unknown readiness/error fields are preserved rather than filtered by allowlist.
- Metadata aims for 16 KiB before MCP text/structured compatibility wrapping.
  Large element lists are paged with returned/total/next_offset and native
  coverage separately reported. An over-budget non-element envelope is explicitly
  marked rather than silently discarding unknown fields. This is not a universal
  model-token limit or hard allocation bound.

Callers must inspect `presentation.elements.next_offset`. An incomplete displayed
page is not native absence. Do not use a repeated observation to retrieve omitted
diagnostics, and never replay input to reconstruct a past result.

## Retained original metadata

`presentation.result_ref` belongs only to that adapter process. It is **not** an
execution or cancellation identity. At most 64 entries and a 64 MiB serialized
metadata budget are retained for up to 300 seconds; memory overhead is not an
allocator-hard bound. Pressure can evict earlier, and adapter exit loses the
cache. Native execution/durable request state is independent of this retention.

The cached object is the original metadata after existing JS-safe integer
normalization. Its canonical serialized SHA-256 is supplied. Image bytes are
not duplicated in this metadata cache; existing image/artifact tools still apply.

Examples of `computer.result` arguments:

```json
{"result_ref":"returned reference","pointer":"/scene/elements","offset":56,"limit":16}
```

```json
{"result_ref":"returned reference","pointer":"/operation_ref"}
```

```json
{"result_ref":"returned reference","pointer":"","byte_offset":0}
```

Array pages have limit 1..64 and an 8 KiB value budget. Oversized objects return
bounded child keys to guide JSON Pointer selection. `byte_offset` instead exports
any selected value as UTF-8 JSON text chunks (up to 4 KiB before wrapping), with
total bytes, next_byte_offset and a selected-value SHA-256. Concatenate chunks
and parse JSON only after the complete export. Byte offsets must be UTF-8
boundaries and cannot be combined with an array offset.

Expired metadata returns `result_ref_expired`; it must not resolve in another
adapter or be interpreted as proof that a native operation stopped. Use
`computer.operation` with the actual execution/request selector for that purpose.

Both text and structured MCP clients receive the same compact representation
for compatibility. A Host that prints both still duplicates it; prefer one data
representation and emit media separately. This change does not claim that a
native GUI task or model-understanding benchmark has already become faster.
