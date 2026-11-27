#!/usr/bin/env bash
# Build a .deb from a staged tree. Usage:
#   tools/build_deb.sh NAME VERSION STAGE
# The stage must already contain the files to install (usr/..., etc.).
set -euo pipefail
name="${1:?usage: build_deb.sh NAME VERSION STAGE}"
version="${2:?usage: build_deb.sh NAME VERSION STAGE}"
stage="${3:?usage: build_deb.sh NAME VERSION STAGE}"
mkdir -p "$stage/DEBIAN"
cat > "$stage/DEBIAN/control" <<EOF
Package: $name
Version: $version
Section: devel
Priority: optional
Architecture: amd64
Maintainer: Lykaon <release@lykaon-lang.local>
Description: Lykaon compiler, interpreter and runtime
EOF
dpkg-deb --build "$stage" "$name-$version.deb"