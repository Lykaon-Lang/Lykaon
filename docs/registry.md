# Lykaon package registry (M6b)

Status: `IMPLEMENTED AND VERIFIED` (`Lykaon/src/registry.rs`,
`tests/m6b_registry.rs` 3/3 plus the path-safety unit test).

The registry is a local folder (or any shared/self-hosted one). It runs no
network service, downloads nothing, and executes no code at install time.

## Layout

```
registry/
  index.json
  <name>/
    <version>.ship
```

A `.ship` is a gzip-compressed JSON document:

```json
{
  "name": "demo",
  "version": "1.0.0",
  "files": [
    {"path": "Lykaon.toml", "data": "<base64>"},
    {"path": "src/main.lykaon", "data": "<base64>"}
  ]
}
```

`index.json` maps each package to its versions and the SHA-256 of each
`.ship`:

```json
{
  "packages": {
    "demo": {
      "versions": {"1.0.0": "<sha256-hex>"},
      "latest": "1.0.0"
    }
  }
}
```

## Commands

```
lykaon registry status [--registry DIR]
lykaon registry publish <folder> [--registry DIR]
lykaon registry add <name> [dest] [--registry DIR]
```

- `publish` reads `Name is ...`/`Version is ...` from the folder's
  `Lykaon.toml` (the folder name and `0.1.0` are fallbacks), packs every file
  except `.git`, `target` and `registry`, writes `<name>/<version>.ship`,
  computes its SHA-256 and updates `index.json`.
- `add` reads the index, verifies the `.ship` checksum, then extracts it to
  `<dest>/<name>/<version>/` (`dest` defaults to the current folder).
- `status` prints `index.json`.
- `--registry DIR` selects the registry folder; the default is `./registry`.

## Rules

- No install scripts and no code execution at install.
- A checksum mismatch is an error (`checksum mismatch for <name> <version>`)
  and extraction does not happen.
- A package path must stay inside the destination: absolute paths, drive/root
  prefixes and `..` components are refused (`unsafe path`).
- `registry/` is a normal folder, so a registry can live on a shared drive,
  a web server's static files, or a git repository.
- The public repository stays releases-only. A public registry is a separate
  service, not part of the source tree.

## Example

```
# publish the folder ./demo into ./registry
lykaon registry publish demo

# install it into ./out/demo/1.0.0
lykaon registry add demo out

# see what the registry holds
lykaon registry status
```
