#!/bin/sh
# Lykaon Linux installer.
#
# Copies the binaries and the runtime library to a stable place, then links
# the two commands into your PATH. No root and no package manager needed:
#
#   sh install.sh                 install into ~/.local (default)
#   PREFIX=/usr/local sh install.sh   install system-wide (run with sudo)
#
# Uninstall: remove the two links and the lib folder printed at the end.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
prefix=${PREFIX:-$HOME/.local}
libdir="$prefix/lib/lykaon"
bindir="$prefix/bin"

# 1. Install the binaries and the runtime library together. lykaonc-cli
#    finds liblykaon_rt.a next to itself when it builds an executable.
mkdir -p "$libdir" "$bindir"
for f in lykaon-cli lykaonc-cli liblykaon_rt.a; do
    if [ ! -f "$here/$f" ]; then
        echo "missing $f next to install.sh" >&2
        exit 1
    fi
    cp -f "$here/$f" "$libdir/$f"
    chmod 755 "$libdir/$f"
done

# 2. Put the commands on PATH. The links point at the installed copies, so
#    the runtime library stays next to them.
for f in lykaon-cli lykaonc-cli; do
    ln -sf "$libdir/$f" "$bindir/$f"
done

echo "Installed Lykaon to $libdir"
echo "  $bindir/lykaon-cli"
echo "  $bindir/lykaonc-cli"

case ":$PATH:" in
    *":$bindir:"*) ;;
    *)
        echo
        echo "$bindir is not on your PATH. Add this line to ~/.profile or ~/.bashrc:"
        echo "  export PATH=\"$bindir:\$PATH\""
        echo "then open a new terminal."
        ;;
esac

# 3. Native builds need a C toolchain (cc) for the final link step.
if ! command -v cc >/dev/null 2>&1; then
    echo
    echo "Note: 'lykaonc-cli build' needs a C compiler for the final link."
    echo "Install one (for example: sudo apt install build-essential)."
fi

cat <<'EOF'

Try it:
  printf 'Say Hello world.\n' > hello.lykaon
  lykaon-cli run hello.lykaon
  lykaonc-cli build fast hello.lykaon hello
  ./hello

No Rust toolchain is needed to run or build this release.
EOF
