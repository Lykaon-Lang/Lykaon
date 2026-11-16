# Lykaon

**Lykaon** is a programming language where you write programs in plain English sentences.
A sentence ends with a period. A block is indented. There are no quotes, no braces,
no semicolons. The compiler reads your sentences and builds a real native executable.

[![CI](https://github.com/Lykaon-oss/Lykaon/actions/workflows/ci.yml/badge.svg)](https://github.com/Lykaon-oss/Lykaon/actions/workflows/ci.yml)

Lykaon is **source-available, not open source**: free for personal use and for
organizations under USD 1,000,000 annual revenue. See [`LICENSE.md`](LICENSE.md).
Programs you write in Lykaon are yours to license and sell.

This repository holds Lykaon **version 0.1 (milestones M1–M4)**: the language core,
the interpreter, the native compiler, the runtime library, and ready-to-run examples.

```lykaon
Say Hello World.
```

```
> lykaon-cli run hello.lykaon
Hello World
```

or build a real executable:

```
> lykaonc-cli build fast hello.lykaon hello.exe
> hello.exe
Hello World
```

## What is inside version 0.1

| Area | What you get |
|---|---|
| M1 — Language core | Sentences, blocks, `Changeable`/`Constant`/`Set`, `Say`, `Ask`, `If`/`Otherwise`, `Repeat`, `For each`, math and comparison words, a formatter, and precise error messages (`Where`/`Why`/`Fix`) |
| M2 — Data and ownership | Lists, maps, sets, pairs, text in depth, records, enums, `maybe`/`missing`, `can fail`/`Try`/`Fail with`, modules (`Use`, `Share`), tests (`Test`, `Check`, `Pretend`), ownership (`give`, `lend`, `copy of`, `share`, `weak`) |
| M3 — Native code | The Cranelift-backed compiler: `lykaon build` in debug/fast/small modes, an optimizer (inlining, dead-code removal, bounds-check elimination, devirtualization, whole-program passes), generics, interfaces, closures, generators, contracts (`Requires`, `Ensures`, `Always`), packages (`Lykaon.toml`, `lykaon.lock`, `lykaon add/remove/update`), `lykaon bench` |
| M4 — Servers and concurrency | Tasks (`Start task`, `Wait for`, `Do these together`), channels, guarded/atomic/counter values, timers (`After`, `Every`), worker pools, files and folders, zip/gzip, JSON/TOML/YAML/XML/CSV read and write, dates and time zones, random, hashing, secrets, `Run`/`Exit`/signals, logging and settings, HTTP client and server with TLS, templates, sessions and WebSocket, command-line tools (`Takes`, `Option`, help text), git dependencies |

In v0.1 a `.lykaon` program builds to a **command-line / console executable** on Windows
(proven by the test suite) and Linux (in principle; not yet verified on Linux hardware —
see *Known limits*). Desktop/windowed apps arrive in a later version.

## Download

Prebuilt Windows x64 binaries (CLI + GUI + runtime) are attached to the
[v0.1.0 release](../../releases/tag/v0.1.0). On Linux, build from source with
the Rust toolchain (1.85 or newer):

```
cargo build --release -p lykaon-rt
cargo build --release -p lykaon
```

(The runtime library is built on its own so it stays TLS-free; see
`Lykaon/src/linker.rs`.)

## Try it

Build from source (you need [Rust](https://rustup.rs) 1.85 or newer):

```
cargo build --release
```

The interpreter:

```
lykaon-cli run hello.lykaon
lykaon-cli repl
```

The compiler:

```
lykaonc-cli build fast hello.lykaon hello.exe
hello.exe
```

## Documentation

| File | Contents |
|---|---|
| [`docs/syntax.md`](docs/syntax.md) | The 5-minute tour: variables, control flow, functions, modules |
| [`docs/standard-library.md`](docs/standard-library.md) | The built-in words, grouped by task |
| [`docs/cli.md`](docs/cli.md) | Every command: run, repl, build, test, add, update, bench |
| [`docs/Lykaon_Language_Reference.docx`](docs/Lykaon_Language_Reference.docx) | The complete language reference for v0.1 (M1–M4), in one document |

## Repository layout

```
Lykaon/            the language workspace
  core/            lexer, parser, interpreter, MIR, codegen, net, formats
  rt/              lykaon-rt, the runtime library linked into built programs
  src/             the two front ends (lykaon-cli, lykaonc-cli), linker, packages
  benchmarks/      the six standard benchmark programs + Rust reference sources
LICENSES/          third-party license texts
logos/             project logos
```

## Status and limits (read this before you rely on it)

- **Windows**: everything above is tested and passing on Windows x86-64.
- **Linux**: built and smoke-tested on every push by CI (ubuntu-latest); the
  full stress suite has not been run on Linux hardware yet.
- **macOS**: not a target. The language targets Windows and Linux x86-64
  (later versions may add macOS).
- **Performance**: not yet a focus. The LLVM backend and the real standard
  benchmarks (nbody, fannkuch, spectral_norm, binary_trees, mandelbrot,
  json_parse) are planned for later versions; the bundled benchmark is a
  recursion stress test, not one of those.
- **Windowed apps**: not in v0.1. Only command-line and console programs.

## License

Lykaon is **source-available, not open source**. It is free for personal use and for
organizations under USD 1,000,000 annual revenue; see [`LICENSE.md`](LICENSE.md) for
the exact terms. Third-party components are listed in [`THIRD_PARTY.json`](THIRD_PARTY.json)
and [`LICENSES/`](LICENSES/).

Programs you write in Lykaon are **yours** — you may license and sell them freely.