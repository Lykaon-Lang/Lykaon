# Standard-library audit (M6b)

Status: `IMPLEMENTED AND VERIFIED` (audit). Each row was checked against the
implemented grammar/semantics in `Lykaon/src/sem.rs` and `Lykaon/core/src`.
"Present" means a real Lykaon sentence or value kind exists; "gap" is what a
Python user would still miss.

Ranked by real-world Python use. Only the genuine gaps need building.

| # | Python module | Lykaon today (verified) | Gap |
|---|---|---|---|
| 1 | `math` | operators `plus`/`minus`/`times`/`divided by`/`to the power of` | **math functions**: `sqrt`, `floor`, `ceil`, natural logarithm, `pow` |
| 2 | `json` | present: `format json` parse/serialize (sem.rs 5533) | none |
| 3 | `datetime` | present: `today`, `now` (sem.rs 11674/11685), `Date`/`Moment`/`Duration`, `days between` | none major |
| 4 | `random` | present: `Random`, `random key` (sem.rs 10151/11855) | none |
| 5 | `os` | present: files/folders, `environment` (sem.rs 10184), `Make` folder | none major |
| 6 | `sys` | present: args, `Exit` | none |
| 7 | `re` | present: `split`, `Replace` (literal, sem.rs 12824) | **regex patterns** (only literal split/replace) |
| 8 | `statistics` | present: `average`/`median`/`total`/`smallest`/`biggest`/`spread` (sem.rs 9127, data.rs 258) | `mode`, `stdev` |
| 9 | `collections` | present: `Counter` kind (sem.rs 498), `Map`/`Set`/`List` | `defaultdict` |
| 10 | `csv` | present: `format csv` (sem.rs 6623) | none |
| 11 | `hashlib` | `Hash` sentence present | **selectable `sha256`/`md5`** (only internal trace uses sha256) |
| 12 | `itertools` | present: `Apply`, `Combine`, `For each` | none major |
| 13 | `functools` | present: `Apply each` | none |
| 14 | `secrets` | present: `Random`, `Secret`, `random key` | none |
| 15 | `base64` | absent | **encode/decode base64** |
| 16 | `struct` | absent | **pack/unpack** |
| 17 | `sqlite3` | present: `Open database`, `db` | none |
| 18 | `http` | present: `Fetch`/`Post`/`Serve` | none |
| 19 | `socket` | present: `Serve`, channels | raw socket sentences |
| 20 | `subprocess` | `Run` present | **process output capture** |

## Real build list — DONE (M6b built-ins)

All nine gaps are implemented as real, tested built-ins (`core/src/builtins.rs`,
`tests/m6b_builtins.rs` 9/9). Call them by name; multi-argument calls use
`with` (or `and` where the parser groups the arguments).

1. `math`: `sqrt`, `floor`, `ceiling`, `logarithm` (natural log), `power` (pow).
2. `re`: `regexreplace` (text with pattern with replacement), `regexsplit`
   (text with pattern) — a small backtracking engine (`. * + ? [] ^ $ \d \w \s`).
3. `hashlib`: `sha256`, `md5` (hex text).
4. `base64`: `b64encode`, `b64decode` (can fail).
5. `struct`: `packwhole`/`unpackwhole`, `packdecimal`/`unpackdecimal`
   (8-byte little-endian).
6. `subprocess`: `capture` (runs a command, returns stdout; can fail).
7. `collections`: `mapdefault` (map with key with default).
8. `socket`: `tcpopen` (host with port), `tcpsend`, `tcpclose`.
9. `statistics`: `mode`, `stdev`.

Note: the book's text literals are bare words, so source-level patterns cannot
contain symbols; use the built-ins directly (or a plain-word pattern) for regex.
The native default backend does not include these; use `run --interpret`.