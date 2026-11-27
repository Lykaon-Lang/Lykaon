# Lykaon file extensions

| File type | Extension | Meaning |
|---|---|---|
| Source code | `.lykaon` | Lykaon source |
| Project configuration | `Lykaon.toml` | Project settings |
| Dependency lockfile | `lykaon.lock` | Locked dependencies |
| Library collection | `.shelf` | A collection of libraries |
| Package file | `.ship` | Distributable package |
| Compiled bytecode | `.byte` | Compiled bytecode |
| Game assets | `.game` | Game-specific asset collection |
| Resources | `.resc` | General resource collection |
| C stub declarations | `.stub.shelf` | C ABI declarations for a library |

Old names are normalized by the compiler and tools:

- `.nest` → `.resc`
- package `.zip` → `.ship`
- `.stub.tern` → `.stub.shelf`
- `.screens`, `.sky`, `.canvas` are **not Lykaon extensions**; they belong to
  the separate Screens/Element projects.