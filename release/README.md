# Lykaon 0.1.0 (Windows x64)

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
lykaonc-cli run hello.lykaon
```

```
Hello world
```

If the terminal says `lykaonc-cli is not recognized`, use the full path:

```
"%LOCALAPPDATA%\Lykaon\lykaonc-cli.exe" run hello.lykaon
```

Build a standalone executable:

```
lykaonc-cli build fast hello.lykaon hello.exe
hello.exe
```

On Windows, double-clicking `hello.exe` keeps the window open until Enter.

## VS Code

`install.bat` installs the extension. Open a `.lykaon` file and press `Ctrl+F5`,
or click **Run File** in the editor toolbar. Both are editor commands, not
terminal commands.

To add the wolf icon in the Explorer, pick the theme once:
Command Palette (`Ctrl+Shift+P`) -> **Preferences: File Icon Theme** -> **Lykaon**

To install the extension by hand instead:

```
code --install-extension "C:\full\path\to\lykaon-0.1.0.vsix"
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
- `lykaon-0.1.0.vsix` - the VS Code extension.

## Command line

```
usage: lykaonc-cli <command> [args]
  run file.lykaon | check file.lykaon | fmt file.lykaon | test [file]
  build file.lykaon | repl | bench | report
  lsp | debug file.lykaon | doc | outline | explain | fix
  migrate [status] | setup | serve | sbom
  checkpoint file.lykaon | resume file.lykaon
  add <folder-or-url> | remove <name> | update
```
