# Lykaon 0.3.0 (Windows x64)

Lykaon is a programming language you read like plain English. The compiler turns
sentences into a standalone native executable.

## Install

1. Unzip this folder anywhere.
2. Double-click `install.bat`.

That copies the tools to `%LOCALAPPDATA%\Lykaon`, adds that folder to your
User PATH, registers the `.lykaon` file icon, and installs the VS Code
extension when VS Code is found.

3. Open a **new** terminal. Windows only refreshes PATH for windows opened
   after the install.

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

If the terminal says `lykaon-cli is not recognized`, use the full path:

```
"%LOCALAPPDATA%\Lykaon\lykaon-cli.exe" run hello.lykaon
```

Build a standalone executable:

```
lykaonc-cli build fast hello.lykaon hello.exe
hello.exe
```

On Windows, double-clicking `hello.exe` keeps the window open until Enter.

## What is new in 0.3.0

This release is M1-M6 + M6b:

- C bridge: `Use C library NAME.`, `Share for C.`, `.stub.shelf` declarations,
  `as C does`, `packed` and `aligned to N`.
- Python extension modules: `lykaonc-cli build for python program.lykaon`.
- JavaScript/WebAssembly: `lykaonc-cli build for js program.lykaon`.
- C-ABI plugins: `Load plugin NAME.`.
- C library target: `lykaonc-cli build for library program.lykaon`.
- Local package registry: `lykaon registry publish|add|status`.
- `lykaon bindgen header.h` for C prototypes.
- Nine standard-library built-ins (math, hashing, base64, struct, process
  capture, map default, regex, raw TCP, mode/stdev).

The bridges, plugins, registry and built-ins run on the interpreter path
(`lykaon-cli run --interpret program.lykaon`); native builds of programs that
use them fall back to the embedded evaluator.

## VS Code

`install.bat` installs the extension. Open a `.lykaon` file and press `Ctrl+F5`,
or click **Run File** in the editor toolbar. Both are editor commands, not
terminal commands.

To add the wolf icon in the Explorer, pick the theme once:
Command Palette (`Ctrl+Shift+P`) -> **Preferences: File Icon Theme** -> **Lykaon**

To install the extension by hand instead:

```
code --install-extension "C:\full\path\to\lykaon-0.3.0.vsix"
```

Syntax highlighting, snippets, Run/Test, diagnostics, hover, completion,
go-to-definition, formatting and F5 debugging all work.

## What is in this folder

- `lykaonc-cli.exe` - compiler + CLI tools.
- `lykaon-cli.exe` - interpreter / REPL.
- `lykaon_rt.lib` - runtime library that `lykaonc-cli` links into native builds.
- `lykaon.ico` - the Lykaon file icon (a document with the wolf inside).
- `install.bat` - double-click to install.
- `install.ps1` - the installer script (`install.bat` runs it).
- `lykaon-0.3.0.vsix` - the VS Code extension.

## Command line

```
usage: lykaonc-cli <command> [args]
  run file.lykaon | check file.lykaon | fmt file.lykaon | test [file]
  build <mode> file.lykaon out | build for library|python|js file.lykaon [outdir]
  repl | bench | report | lsp | debug file.lykaon | doc | outline | explain | fix
  bindgen header.h [out.stub.shelf]
  registry status | registry publish <folder> | registry add <name> [dest]
  migrate [status] | setup | serve | sbom
  checkpoint file.lykaon | resume file.lykaon
  add <folder-or-url> | remove <name> | update
```

## Linux

A prebuilt Linux x86-64 archive is published as
`Lykaon-v0.3.0-linux-x64.tar.gz`. Unpack it and run `install.sh`; no Rust is
needed. See its README for details.

## Verify the download (optional)

The zip is sealed so you can check it was not changed after we built it. You do not need this to use Lykaon.

- `SHA256SUMS` lists the SHA-256 of the zip, the VS Code extension and the SBOM.
- `Lykaon-v0.3.0-windows-x64.zip.sig` is the seal for the zip.
- `lykaon-0.3.0.cdx.json.sig` is the seal for the SBOM.
- `SHA256SUMS.sig` is the seal for the list.
- `lykaon-release.pub` is the public key.

Quick check in PowerShell (nothing to install):

```
Get-FileHash .\Lykaon-v0.3.0-windows-x64.zip -Algorithm SHA256
```

The hash must match the zip line in `SHA256SUMS`.

Full check of the seal (needs OpenSSL; Git for Windows already includes it):

```
openssl dgst -sha256 -verify lykaon-release.pub -signature Lykaon-v0.3.0-windows-x64.zip.sig Lykaon-v0.3.0-windows-x64.zip
```

```
Verified OK
```