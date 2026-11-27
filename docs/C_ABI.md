# LYKAON stable C ABI (v1 draft)

This is the Model B boundary for Element/Screens and for C/C++/Rust/Python/JS
hosts. It is additive: a program that never crosses the ABI pays nothing.
Status: **implemented** in the `Lykaon/ffi` crate (C ABI v1).

## Rules

- The ABI is C-compatible, versioned, and never guesses.
- The host owns all buffers; the library never returns a pointer the host must
  free unless a `_free` function is documented.
- No secret values cross the ABI.
- A host that does not support the ABI version refuses it with Where/Why/Fix.

## Functions

```c
// Version of the ABI (currently 1).
uint32_t lykaon_abi_version(void);

// Evaluate Lykaon source. `out` is a caller buffer of `cap` bytes.
// Writes at most cap-1 bytes, null-terminates, and stores the needed length
// in *out_len. Returns the Lykaon exit code (0 = ok).
int32_t lykaon_eval(
    const char *src,
    char *out,
    size_t cap,
    size_t *out_len
);
```

## Value mapping

Payload values use the M4 JSON conventions (the Element/Screens integration
note 4.3): whole -> integer, decimal ->
number, text -> string, yes/no -> boolean, maybe -> null, list -> array,
record -> object, enum -> `{"kind": variant, ...}`, date/moment -> ISO 8601,
secret never crosses.

## Ownership and errors

- `lykaon_eval` copies the program output into the caller's buffer.
- Exit code 0 = success; 1 = program error; 2 = ABI usage error.
- The host must keep `src` and `out` alive for the call.

## Relationship to Element

- Model A (recommended first): JSON lines over stdio or WebSocket; no ABI.
- Model B (later): Element loads this library; the same event/command JSON
  protocol flows through `lykaon_eval` calls or a dedicated call ABI.
- The ABI version is independent of the Screens IR version and the protocol
  `v`.