# Lykaon release (Windows)

- `lykaonc-cli.exe` — compiler + LSP + tools.
- `lykaon-cli.exe` — interpreter / REPL.
- `lykaon.ico` — the Lykaon wolf icon.
- `install.bat` — double-click to install (copies to `%LOCALAPPDATA%\Lykaon`, adds to PATH, registers the icon).
- `install.ps1` — the installer script (install.bat runs it).
- `lykaon-0.1.0.vsix` — the VS Code extension.

## Install

1. Unzip this folder anywhere.
2. **Double-click `install.bat`** (or run `install.ps1`).
   - Copies the binaries + icon to `%LOCALAPPDATA%\Lykaon`.
   - Adds that folder to your User PATH.
   - Registers `.lykaon` so File Explorer shows the wolf icon.
   - Refreshes Explorer immediately.
3. Open a new terminal and run:
   ```
   lykaonc-cli run hello.lykaon
   ```

You can now delete the Downloads/unzip folder — the install lives in `%LOCALAPPDATA%\Lykaon`.

## VS Code

```
code --install-extension lykaon-0.1.0.vsix
```
Open a `.lykaon` file: highlighting, diagnostics, hover, completion, definition,
symbols, references, rename, format, Run/Test and debugging work. The extension
offers to switch to the Lykaon file-icon theme on first use.