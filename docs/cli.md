# The Lykaon command line (v0.3)

Two programs come with a release:

| Program | Role |
|---|---|
| `lykaon-cli` (`lykaon-cli.exe` on Windows) | the interpreter front end: run, repl, ui, package and registry commands |
| `lykaonc-cli` (`lykaonc-cli.exe` on Windows) | the compiler front end: build native executables and library targets |

## Running programs

```
lykaon-cli run program.lykaon          run a program
lykaon-cli run --native program.lykaon run through the native path
lykaon-cli repl                        interactive read-eval-print loop
lykaon-cli ui                          open the compiler UI (GUI build)
lykaon-cli test program.lykaon         run the Test sentences in a file
```

- `Ask` reads from the keyboard when stdin is a terminal; when stdin is not a
  terminal, `Ask` fails with *no person here* and `For each line in input.`
  streams stdin instead — this is how pipe-friendly tools are written.

## Building executables

```
lykaonc-cli build <mode> input.lykaon output
```

| Mode | Meaning |
|---|---|
| `debug` | fresh tree, no optimizer passes — closest to the interpreter |
| `fast` | optimizer passes applied, tuned for speed |
| `small` | optimizer passes applied, tuned for size |

Example:

```
lykaonc-cli build fast hello.lykaon hello.exe
```

`build` prints the optimizer report (contract prover, constant folding, inlining,
dead code, bounds-check elimination, devirtualization, whole-program passes) —
one line per pass with its change count.

### Library and bridge targets (M6)

```
lykaonc-cli build for library input.lykaon [outdir]   # writes <stem>.h + <stem>.nest
lykaonc-cli build for python  input.lykaon [outdir]   # writes a Python extension
lykaonc-cli build for js      input.lykaon [outdir]   # writes <stem>.wasm + loader + .d.ts
```

- `build for library` exports the functions marked `Share for C.` as a C
  header plus an export manifest (`.nest`).
- `build for python` writes a Python extension module that embeds and runs the
  program (`import <stem>; <stem>.run()`). It needs the Python development
  headers and a C toolchain.
- `build for js` writes the interpreter as WebAssembly plus an ES-module
  loader; a browser or Node calls `run()`.

The C/Python/JavaScript bridges, plugins and the M6b built-ins run on the
interpreter path. A native build of a program that uses them falls back to the
embedded evaluator, as documented in `docs/C_ABI.md` and
`docs/unsafe-boundary.md`.

The output is a standalone executable for the host platform: `.exe` on Windows,
an ELF binary on Linux. Double-clicked in Explorer it keeps its window open
until you press Enter; run from a terminal it prints and returns to the
prompt. It embeds what it needs and finds the runtime library
(`lykaon_rt.lib` / `liblykaon_rt.a`) by searching its own folder, its ancestors,
`target/<profile>`, `Lykaon/target/<profile>` and `rt/target/<profile>` — so a
build run from anywhere in the workspace works. To point at a specific library,
set the environment variable:

```
set Lykaon_RT_LIB=C:\path\to\lykaon_rt.lib
```

## Command-line tools made in Lykaon

A `.lykaon` file that declares `Takes`/`Option` sentences *is* a command-line tool
once built:

```lykaon
Takes file name as text.
Option verbose is whole number.
Option log level is text defaulting to debug.

If verbose is at least 1.
    Log info starting.
Say Read file name in temporary folder.
```

```
> tool.exe --help
usage + option help, exit code 0
> tool.exe
usage text, exit code 2          (a missing Takes argument is never missing)
> tool.exe --verbose 2 --log-level info data.txt
```

- Positional arguments bind to `Takes` in order
- `--name value` (or `--name=value`) fills an `Option`; a bare `--flag` is the
  word `on` for text options
- `--help` as the first argument prints the usage text and exits 0

## Packages

A package folder holds a `Lykaon.toml`:

```lykaon
Name is myapp.
Version is 1 point 0 point 0.
Needs helpers from folder helpers.
Needs tools from git https://example.com/tools.git at version 1 point 2 point 0.
```

```
lykaon-cli add <folder-or-url>       add a dependency (folder or git URL)
lykaon-cli remove NAME               remove it
lykaon-cli update [NAME]             clone/fetch, check out the pinned tag,
                                     verify the checksum, refresh lykaon.lock
```

- Git deps are cloned into `.lykaon-cache/<name>` and pinned by tag
  (`vX.Y.Z` or `X.Y.Z`); the lock records a SHA-256 tree checksum and refuses
  a rewritten tag (`Git dependency failed (T0520)`)
- `lykaon.lock` is written and updated automatically; commit it

## Local package registry (M6b)

```
lykaon registry status [--registry DIR]
lykaon registry publish <folder> [--registry DIR]
lykaon registry add <name> [dest] [--registry DIR]
```

- `publish` packs a folder into `<name>/<version>.ship` and records its SHA-256
  in `index.json`; `add` verifies the checksum and extracts to
  `<dest>/<name>/<version>/`; `status` prints the index.
- The registry is a local or self-hostable folder: no network service, no
  downloads and no code execution at install.
- A checksum mismatch or a package path that escapes the destination is
  refused. Details: `docs/registry.md`.

## C headers: bindgen

```
lykaon bindgen header.h [out.stub.shelf]
```

Reads simple C prototypes (`int`, `long long`, `double`, `char *`, `void`) and
writes Lykaon `C function ... as ... takes ... gives ...` declarations (a
`.stub.shelf`) for the C bridge. With no output file the text goes to stdout.

## Benchmarks

```
lykaon-cli bench
```

Runs the benchmark programs in `Lykaon/benchmarks/` — compile time measured
separately from execution, median of 10 runs with a warm-up, and a checksum of
the program's output printed on every run so you can verify results yourself.

The bundle contains one honest recursion stress test (`recursion.lykaon`, naive
Fibonacci) and its Rust reference source (`reference/recursion.rs`). `lykaon
bench` needs the Rust reference executable built first:

```
rustc -O reference/recursion.rs -o reference/bin/recursion.exe   (Windows)
rustc -O reference/recursion.rs -o reference/bin/recursion       (Linux)
```

The stored times in `rust_reference.json` are a staleness cross-check only;
ratios are measured live against the executable in the same run. The real
standard benchmarks (nbody, fannkuch, spectral_norm, binary_trees, mandelbrot,
json_parse) are planned for a later version.

## Building from source (optional)

A prebuilt Linux archive (`Lykaon-v0.3.0-linux-x64.tar.gz`) needs no Rust or
Cargo: unpack it and run `install.sh`. The steps below are only for building
from the source tree.

```
cargo build --release -p lykaon-rt
cargo build --release -p lykaon
```

executables land in `target/release/`. (The runtime library is built on its
own so it stays TLS-free.)

## Note on the two front ends

`build` belongs to `lykaonc-cli`. If you ask `lykaon-cli` to build, it reminds you:

```
the lykaon front runs the interpreter: lykaon run <file> | lykaon repl | lykaon ui
the compiler front is lykaonc-cli (build belongs there)
```
