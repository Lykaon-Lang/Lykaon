# Unsafe boundaries (M6b)

Status: `IMPLEMENTED AND VERIFIED`. A boundary is any place where Lykaon calls
code or data it does not control. Every boundary is explicit, converts kinds at
the edge, and turns a failure into a normal Lykaon failure (never a silent
workaround).

## The rule

- Run with `lykaon run safely <file>` (or `lykaon run --safely <file>`) to
  enable the sandbox. Without it the program runs unsandboxed.
- The sandbox enforces the `Allows` permissions in `settings.lykaon`. A boundary
  that needs a permission the program does not have is refused with **T0622**
  ("Sandbox refused. Add the Allows permission or remove the sentence.").
- A boundary failure is a normal failure (T0525 family), never a fallback that
  hides the error.
- The interpreter path (`run --interpret` / `run safely`) is the path that has
  the bridges and built-ins; the native default backend does not.

## Boundaries and the permission each needs

| Boundary | Surface | Permission | Failure |
|---|---|---|---|
| C bridge | `Use C library NAME.` + `.stub.shelf` | (in-process; no sandbox gate) | T0525 |
| Python bridge | `.py.shelf` (`Python function ...`) | (in-process; no sandbox gate) | T0525 |
| Subprocess | `capture <command>` | audit: `Allows processes` (`perm-process`) | T0525 |
| Raw TCP | `tcpopen` / `tcpsend` | audit: `Allows network` (`perm-network`) | T0525 |
| File access | `Read`/`Write`/`Append`/`Load` | `Allows files` (runtime T0622) | T0622 |
| Environment | `environment` | `Allows environment` | T0622 |
| Decoders | `b64decode`, `unpackwhole`, `unpackdecimal` | none (pure, but can fail) | T0525 |

The runtime sandbox (`run safely`) intercepts file/network/process *statements*
and refuses with T0622. The built-in calls above are gated by the audit
(`lykaon audit`): `capture` raises `perm-process`, `tcpopen`/`tcpsend` raise
`perm-network` when the matching `Allows` is missing.

## C bridge: `safe` and `Be careful.`

```
C safe function cos as cosine takes decimal gives decimal.
C function strlen as length takes text gives whole number.
```

- `safe` means the function may be called anywhere.
- Without `safe`, the call must be inside `Be careful.` — the explicit
  "I accept the risk" block.
- No raw pointers cross: the boundary converts Lykaon kinds to C ABI kinds and
  back (scalars, text, and `#[repr(C)]` records).

## Python bridge

```
# math.py.shelf
Python function sqrt as pysqrt takes decimal gives decimal.
```

- The Python C API is resolved dynamically at run time (no link-time Python
  dependency).
- Values are converted at the edge (whole/decimal/yes-no/text); a Python
  exception becomes a normal failure with the exception message.
- The module is named by the file stem (`math.py.shelf` → `math`).

## Built-in boundaries

- `capture` runs a shell command and returns stdout; it needs
  `Allows processes` under `run safely`.
- `tcpopen`/`tcp_send`/`tcpclose` open a raw TCP connection; they need
  `Allows network` under `run safely`.
- `b64decode`, `unpackwhole`, `unpackdecimal` are pure but can fail on bad
  input; they report a normal failure.
- `regexreplace`/`regexsplit` run a small backtracking matcher — bounded,
  CPU-only, no I/O.

## What is never allowed

- No install-time or package code execution (the registry only unpacks files).
- No silent catch: a boundary error is always surfaced.
- No raw pointer or unchecked `unsafe` across the language edge.