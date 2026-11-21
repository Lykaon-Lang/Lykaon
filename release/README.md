# Lykaon release (Windows)

- `lykaonc-cli.exe` — compiler + LSP + tools (`check`, `run`, `build`, `audit`, ...).
- `lykaon-cli.exe` — interpreter / REPL.
- `lykaon.ico` — the Lykaon wolf icon.
- `install.ps1` — registers the `.lykaon` file type + icon (run once).
- `lykaon-0.1.0.vsix` — the VS Code extension (install or publish).

## Quick start

1. Unzip this folder anywhere.
2. Run `install.ps1` once (double-click it, or
   `powershell -ExecutionPolicy Bypass -File install.ps1`).
   - This makes File Explorer show the wolf icon on `.lykaon` files.
3. Put the folder on `PATH` (or use the full path) to run `lykaonc-cli`.
11. Open a `.lykaon` file and run it:
   ```
   lykaonc-cli run hello.lykaon
   ```

## VS Code

Install `lykaon-0.1.0.vsix`:
```
code --install-extension lykaon-0.1.0.vsix
```
Then open a `.lykaon` file: highlighting, diagnostics, hover, completion,
definition, symbols, references, rename, format, Run/Test and debugging work.
The extension offers to switch to the Lykaon file-icon theme on first use.