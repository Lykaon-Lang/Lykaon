# Lykaon in Neovim

## 1. File type + highlighting

Copy `ftdetect/lykaon.vim` into your Neovim runtime (e.g. `~/.config/nvim/ftdetect/`),
and `queries/lykaon/highlights.scm` into `~/.config/nvim/queries/lykaon/`
(or install the tree-sitter parser from `editors/tree-sitter-lykaon`).

## 2. LSP (diagnostics, hover, completion, definition)

With `nvim-lspconfig`:

```lua
require('lspconfig').lykaon.setup {
  cmd = { 'lykaonc-cli', 'lsp' },
  filetypes = { 'lykaon' },
  root_dir = require('lspconfig.util').root_pattern('Lykaon.toml', '.git'),
}
```

Or, without lspconfig, start `lykaonc-cli lsp` manually as a language server.

## 3. Run / test

Add keymaps that call the CLI:

```lua
vim.keymap.set('n', '<leader>r', ':!lykaonc-cli run --interpret %<CR>')
vim.keymap.set('n', '<leader>t', ':!lykaonc-cli test %<CR>')
```