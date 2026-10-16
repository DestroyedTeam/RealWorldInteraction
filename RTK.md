# RTK command wrapper

Use `rtk` for shell commands in this repository. Prefix each command in a chain.
Use `rtk proxy <command>` when a native filter does not support the needed flags
or when exact diagnostic output is required.

Examples:

```sh
rtk git status
rtk proxy rg -n 'symbol' crates/rwi-core/src
rtk cargo fmt --all -- --check
rtk cargo test --workspace --locked --offline
rtk cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
```

This file satisfies the `AGENTS.md` include. Runtime, native-window, Git and
indexing permissions remain governed by `AGENTS.md` and the current user request.
