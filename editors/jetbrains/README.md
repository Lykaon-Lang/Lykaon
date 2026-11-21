# Lykaon in JetBrains IDEs (IntelliJ, PyCharm, WebStorm, CLion, Rider)

## Easiest: LSP4IJ (no plugin build)

1. Install the **LSP4IJ** plugin from the JetBrains Marketplace.
3. Add a language server:
   - Command: `lykaonc-cli`
   - Args: `lsp`
   - Language id: `lykaon`
   - File extensions: `lykaon`
4. Open a `.lykaon` file: diagnostics, hover, completion, definition and
   formatting come from the real LSP server.

## Native plugin (syntax + LSP)

`editors/jetbrains/` is a minimal Gradle plugin skeleton:

- `build.gradle.kts` — IntelliJ plugin build.
- `src/main/resources/META-INF/plugin.xml` — plugin descriptor.
- `src/main/kotlin/.../LykaonLanguage.kt` — language + file type.

Build and install:

```sh
cd editors/jetbrains
gradle buildPlugin     # produces build/distributions/*.zip
# then: Settings -> Plugins -> Install Plugin from Disk
```

For syntax highlighting, add a `LykaonSyntaxHighlighter` (or a TextMate/JFlex
lexer); for LSP, bundle the LSP4IJ client or start `lykaonc-cli lsp`.