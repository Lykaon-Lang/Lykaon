# Lykaon 0.3.0 (Linux x86-64)

Lykaon is a programming language you read like plain English. The compiler
turns sentences into a standalone native executable.

This archive is **prebuilt**: you do not need Rust, Cargo or the source tree to
run Lykaon or to build Lykaon programs. You only need a C compiler (`cc`) when
you build a native executable, because that is the final link step.

- Platform: Linux x86-64 (glibc 2.39 or newer, e.g. Ubuntu 24.04+).
- Built and tested on Ubuntu 26.04 x86-64.
- The native Apple/ARM machines are not part of this archive.

## Install

```
tar xzf Lykaon-v0.3.0-linux-x64.tar.gz
cd Lykaon-v0.3.0-linux-x64
sh install.sh
```

That copies the tools to `~/.local/lib/lykaon` and links
`~/.local/bin/lykaon-cli` and `~/.local/bin/lykaonc-cli` into your PATH.
No root is needed. A system-wide install:

```
sudo PREFIX=/usr/local sh install.sh
```

If `~/.local/bin` is not on your PATH, `install.sh` prints the line to add to
`~/.profile` or `~/.bashrc`. Open a new terminal afterwards.

## Run your first program

Put this in `hello.lykaon`:

```lykaon
Say Hello world.
```

Run it:

```
lykaon-cli run hello.lykaon
```

```
Hello world
```

## Build a standalone executable

```
lykaonc-cli build fast hello.lykaon hello
./hello
```

The result is a normal ELF executable; it does not need the interpreter at run
time. `lykaonc-cli` finds `liblykaon_rt.a` in its own folder, so keep the files
from this archive together.

If `cc` is missing, install a toolchain, for example:

```
sudo apt install build-essential
```

## What is in this folder

| File | What it is |
|---|---|
| `lykaon-cli` | the interpreter: `run`, `repl`, `test`, package and registry commands |
| `lykaonc-cli` | the compiler: `build`, native executables, C/Python/JS/library targets |
| `liblykaon_rt.a` | the runtime library the compiler links into native builds |
| `install.sh` | the installer described above |
| `LICENSE.md`, `LICENSES/` | license terms and third-party licenses |

## What 0.3.0 adds

Version 0.3.0 is the M1-M6 + M6b release:

- **C bridge**: `Use C library NAME.`, `Share for C.`, `.stub.shelf`
  declarations, `as C does`, `packed` and `aligned to N` layouts.
- **Python extension modules**: `lykaonc-cli build for python program.lykaon`
  writes a Python extension that embeds and runs the program.
- **JavaScript / WebAssembly**: `build for js` writes a `.wasm` module and a
  loader.
- **Plugins**: `Load plugin NAME.` loads a C-ABI shared library at run time.
- **C library target**: `build for library` writes a C header and an export
  manifest.
- **Libraries and bridges (M6b)**: a local package registry
  (`registry publish` / `add` / `status`), a Python library bridge
  (`.py.shelf`; calls `math`, `json`, `numpy`, ...), `bindgen` for C headers,
  and nine standard-library built-ins (math, hashing, base64, struct, process
  capture, map default, regex, raw TCP, mode/stdev).

The bridges and built-ins run on the interpreter path (`lykaon-cli run
--interpret`), exactly as documented; native builds of programs that use them
fall back to the embedded evaluator.

## Verify the download (optional)

```
sha256sum Lykaon-v0.3.0-linux-x64.tar.gz
```

Compare it with the SHA-256 printed in the GitHub release notes.

## License

Lykaon is source-available, not open source: free for personal use and for
organizations under USD 1,000,000 annual revenue. See `LICENSE.md`. Programs
you write in Lykaon are yours to license and sell.
