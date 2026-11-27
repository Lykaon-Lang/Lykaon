# Using Lykaon in any IDE

Lykaon ships an **LSP server** (`lykaonc-cli lsp`) and a **tree-sitter grammar**,
so it works in any editor that speaks either — the same way Python, Rust, Go,
Java, etc. do.

## 1. Install the tools

Download the release `lykaonc-cli` (compiler + LSP) and `lykaon-cli` (REPL) for
your platform and put them on `PATH`:

```sh
lykaonc-cli --help      # compiler + lsp + audit + ...
lykaon-cli --help       # interpreter / REPL
```

Every editor below calls `lykaonc-cli lsp` over stdio. Set `lykaon.compilerPath`
(or the editor's LSP command) if the binary is elsewhere.

## 2. Editor by editor

### VS Code
Install `editors/vscode/lykaon-0.3.0.vsix` (or publish it to the Marketplace).
It gives syntax highlighting, LSP diagnostics/hover/completion/definition,
format on save, Run/Test buttons, and DAP debugging.

### Neovim
1. Install the tree-sitter parser: add `editors/tree-sitter-lykaon` as a
   parser (e.g. with nvim-treesitter) and copy `editors/nvim/queries/lykaon`.
2. LSP: `require('lspconfig').lykaon.setup { cmd = { 'lykaonc-cli', 'lsp' } }`.
   (See `editors/nvim/README.md`.)

### Helix
Add to `~/.config/helix/languages.toml`:
```toml
[[language]]
name = "lykaon"
scope = "source.lykaon"
file-types = ["lykaon"]
language-servers = ["lykaon-lsp"]

[language-server.lykaon-lsp]
command = "lykaonc-cli"
args = ["lsp"]
```
(See `editors/helix/languages.toml`.)

### Emacs
```elisp
(load-file "/path/to/editors/emacs/lykaon-mode.el")
;; then open a .lykaon file; M-x lsp-mode if you use lsp-mode
```

### Sublime Text
Install `editors/sublime/lykaon.sublime-syntax` as a package, and point the LSP
client at `lykaonc-cli lsp`.

### JetBrains (IntelliJ / PyCharm / WebStorm / CLion ...)
Install the Lykaon plugin from `editors/jetbrains/` (syntax + LSP), or use the
LSP4IJ plugin and point it at `lykaonc-cli lsp`.

### Any other LSP editor (Zed, Emacs, Kate, etc.)
Configure the language server command: `lykaonc-cli lsp`, language id `lykaon`,
extensions `.lykaon`. Tree-sitter users can use `editors/tree-sitter-lykaon`.

## 3. What you get everywhere

- Syntax highlighting (tree-sitter / TextMate / Sublime).
- Diagnostics with Where/Why/Fix (LSP).
- Hover, completion, go-to-definition, format (LSP).
- Run/Test (VS Code) and DAP debugging (VS Code, `lykaonc-cli debug`).

## 4. Publishing packages

- VS Code: `cd editors/vscode && npx vsce package` -> `.vsix`.
- Open VSX / Marketplace: publish the `.vsix`.
- Tree-sitter: publish `editors/tree-sitter-lykaon` to npm; editors pick it up.
- JetBrains: build the plugin with Gradle and publish to the Marketplace.

## Windows Explorer icons (the wolf on .lykaon files)

Inside VS Code the icon comes from the editor's icon theme. **Windows Explorer**
(the file in a folder) uses a **file association** in the registry — the same
mechanism the Python and .NET installers use. Register it once:

```powershell
lykaonc-cli setup --icon "C:\path\to\lykaon.ico"
```

(or put `lykaon.ico` next to `lykaonc-cli.exe` and run `lykaonc-cli setup`).

This writes, for the current user (no admin):

- `HKCU\Software\Classes\.lykaon` -> `Lykaon.File`
- `HKCU\Software\Classes\Lykaon.File` -> "Lykaon Source File"
- `...\Lykaon.File\DefaultIcon` -> `lykaon.ico,0`
- `...\Lykaon.File\shell\open\command` -> `"lykaon-cli" run "%1"`

After running it, new `.lykaon` files show the **Lykaon file icon** (a document
with the wolf inside) in Explorer.

Notes:

- Windows shows **one icon per file type**. You cannot stack "Lupin Code + Lykaon"
  icons on one file in Explorer; the file-type icon is the wolf, and "Open with"
  (Lupin Code) is a separate association.
- The C# icon you saw is the **.NET SDK's** file association (not a VS Code
  feature); the Python icon is the **Python installer's** association. Lykaon
  does the same via `lykaonc-cli setup`.
- For "by default when downloaded", ship `lykaon.ico` in the release zip and run
  `lykaonc-cli setup` during install (or provide a `register-lykaon.reg`).
- Other IDEs: icons are per-editor (JetBrains file-type icons, `vscode-icons`,
  etc.) and also need a one-time setup.

### Linux (GNOME, KDE, XFCE, ...)

Linux file managers take the icon from the **MIME type**, not an `.ico`. Install
the vector icon once, per user (no root):

```sh
mkdir -p ~/.local/share/mime/packages \
         ~/.local/share/icons/hicolor/scalable/mimetypes
cp docs/assets/lykaon-file.svg \
   ~/.local/share/icons/hicolor/scalable/mimetypes/text-x-lykaon.svg

cat > ~/.local/share/mime/packages/lykaon.xml <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="text/x-lykaon">
    <comment>Lykaon source file</comment>
    <glob pattern="*.lykaon"/>
    <icon name="text-x-lykaon"/>
  </mime-type>
</mime-info>
EOF

update-mime-database ~/.local/share/mime
gtk-update-icon-cache ~/.local/share/icons/hicolor 2>/dev/null || true
```

`docs/assets/lykaon-file.svg` is the flattened vector: some Linux icon loaders
cannot follow the nested SVG-in-`<image>` used by the original artwork.
