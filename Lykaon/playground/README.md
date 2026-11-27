# Lykaon Playground (M5d-preview extra)

The Lykaon interpreter compiled to WebAssembly: edit, run, and see output and
diagnostics entirely in the browser. **Off-plan extra (M5d-preview)** — it
changes no language rule; the real M5d playground is milestone work after M5c.

## Run it locally

Serve this folder over HTTP (the wasm cannot load from `file://`):

```
python -m http.server 8000
```

Then open `http://localhost:8000`.

## Build the wasm

From the workspace root (`Lykaon/`):

```
rustup target add wasm32-unknown-unknown
cargo build --release -p lykaon-playground --target wasm32-unknown-unknown
copy target\wasm32-unknown-unknown\release\lykaon_playground.wasm playground\
```

## Smoke test without a browser

```
node playground\smoke.mjs
```

## Status

| Area | Works | Notes |
|---|---|---|
| Core language (M1–M3): Say, names, control flow, functions, records, enums, generics, generators, contracts | ✅ | the full interpreter runs in wasm |
| M4 tasks, channels, counters, timers (single-threaded eager model) | ✅ | e.g. `Start task ... Wait for` |
| Formats in memory (json of x) | ✅ | `json of` expressions work |
| File sentences (`Read`/`Write`/`in temporary folder`) | ⚠️ graceful failure | no filesystem in the browser sandbox; sentences fail cleanly, no crash |
| Network (`Fetch`, `Serve`) | ⚠️ fails cleanly | no sockets in the browser sandbox |
| `Ask` (keyboard) | ⚠️ fails cleanly | no interactive stdin; use `For each line in input.` with piped text only |
| `random` without `Use seed` | ⚠️ untested | may depend on wasm entropy; prefer `Use seed N.` in playground programs |

## ABI (no wasm-bindgen)

- `lykaon_alloc(len) -> u32` — input buffer (fixed region, 1 MiB max)
- `lykaon_run(ptr, len) -> u64` — `(out_ptr << 32) | out_len`; output text in a
  256 KiB static buffer (truncated beyond that)
- The exported `memory` holds both regions; JS writes source, calls, reads output.

## Layout

- `src/lib.rs` — the wasm ABI (uses `lykaon::run_source`, the same entry the CLI uses)
- `index.html` — the browser UI (dark theme, examples dropdown, Ctrl+Enter)
- `smoke.mjs` — node smoke test