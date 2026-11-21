# Lykaon release (Windows)

- `lykaonc-cli.exe` — compiler + CLI tools.
- `lykaon-cli.exe` — interpreter / REPL.
- `lykaon_rt.lib` — runtime library that `lykaonc-cli` links into native builds.
- `lykaon.ico` — the Lykaon file icon (a document with the wolf inside).
- `install.bat` — double-click to install (copies to `%LOCALAPPDATA%\Lykaon`, adds to PATH, registers the icon).
- `install.ps1` — the installer script (`install.bat` runs it).
- `lykaon-0.1.0.vsix` — the VS Code extension.

## Install

1. Unzip this folder anywhere.
2. **Double-click `install.bat`** (or run `install.ps1`).
   - Copies the binaries, runtime library and icon to `%LOCALAPPDATA%\Lykaon`.
   - Adds that folder to your User PATH.
   - Registers `.lykaon` so File Explorer shows the Lykaon file icon.
   - Refreshes Explorer immediately.
3. Open a new terminal and run:
   ```
   lykaonc-cli run hello.lykaon
   ```
   To build a native executable:
   ```
   lykaonc-cli build fast hello.lykaon hello.exe
   .\hello.exe
   ```

You can now delete the Downloads/unzip folder — the install lives in `%LOCALAPPDATA%\Lykaon`.

## VS Code

```
code --install-extension lykaon-0.1.0.vsix
```

Open a `.lykaon` file: syntax highlighting, snippets, Run/Test and the wolf
file-icon theme work immediately.

Language features that use the language server (diagnostics, hover,
completion, definition, symbols, references, rename, formatting) and
debugging need a build that provides `lykaonc-cli lsp` and `lykaonc-cli debug`.
The public build in this zip may not include those commands yet; the editor
features above still work.
