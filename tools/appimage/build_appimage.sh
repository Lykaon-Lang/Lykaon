#!/usr/bin/env bash
# Build the Linux x64 AppImage from the release binaries.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
app="$root/tools/appimage/appimagetool-x86_64.AppImage"
stage="${1:-/tmp/lykaon-appdir}"
out="${2:-/tmp/Lykaon-v0.2.0-linux-x64.AppImage}"
chmod +x "$app"
rm -rf "$stage"
mkdir -p "$stage/usr/bin"
cp "$root/Lykaon/target/release/lykaonc-cli" "$stage/usr/bin/"
cp "$root/Lykaon/target/release/lykaon-cli" "$stage/usr/bin/"
cp "$root/Lykaon/target/release/lykaon_rt.lib" "$stage/usr/bin/"
cp "$root/editors/vscode/icons/lykaon.png" "$stage/lykaon.png"
cat > "$stage/AppRun" <<'EOF'
#!/bin/sh
exec "$(dirname "$0")/usr/bin/lykaonc-cli" "$@"
EOF
chmod +x "$stage/AppRun"
cat > "$stage/lykaon.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Lykaon
Exec=lykaonc-cli
Icon=lykaon
Categories=Development;
Terminal=true
EOF
cd /tmp
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$app" "$stage" "$out"
ls -l "$out"