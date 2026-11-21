# Lykaon for VS Code

Syntax highlighting, LSP diagnostics, completion, hover, go-to-definition,
format on save, and Run/Test buttons for `.lykaon` files.

## Requirements

- VS Code 1.85+
- `lykaonc-cli` on `PATH` (the language server)
- `lykaon-cli` on `PATH` (Run)
- `lykaonc-cli` on `PATH` (Test)

Set the paths in Settings → Extensions → Lykaon if the tools are elsewhere.

## Features

- Syntax highlighting for `.lykaon`
- Errors with Where/Why/Fix from `lykaonc-cli lsp`
- Completion, hover, go-to-definition
- Format on save
- Run and Test buttons in the editor title

## Packaging

```
cd editors/vscode
npx --yes @vscode/vsce-cli package
```

The result is `lykaon-0.1.0.vsix`. Install with
`code --install-extension lykaon-0.1.0.vsix`.

The extension is plain JavaScript; there is no build step.
## Debugging

The extension registers a `lykaon` debug adapter that runs `lykaonc-cli debug
<file>` and speaks the Debug Adapter Protocol to it over stdio.

1. Open a `.lykaon` file.
2. Press F5 (Run and Debug) and choose **Debug Lykaon file** (or create a
   launch config with `"type": "lykaon"`, `"request": "launch"`,
   `"program": "${file}"`).
3. Set breakpoints in the gutter, then step (F10/F11), continue (F5), and
   inspect variables in the Run view.

Breakpoints are top-level statement lines (the first cut); stepping pauses at
every statement. Requires `lykaonc-cli` on PATH (or set
`lykaon.compilerPath`).
