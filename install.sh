#!/usr/bin/env bash
# Build notez from this checkout and install it as ~/.local/bin/notez plus
# its argv-0 aliases. Run it on every machine after pulling; the vault layout
# assumes both machines run the same binary.
set -eo pipefail

BOLD=$(printf '\033[1m')
GREEN=$(printf '\033[38;2;166;227;161m')
PEACH=$(printf '\033[38;2;250;179;135m')
RESET=$(printf '\033[0m')

echo ""
echo "  ${BOLD}notez${RESET} installer"
echo ""

if ! command -v cargo &>/dev/null; then
    echo "  ${PEACH}x${RESET} cargo not found. Install Rust: https://rustup.rs"
    exit 1
fi
echo "  ${GREEN}ok${RESET} cargo found"

INSTALL_DIR="${1:-$HOME/.local/bin}"
echo "  ${GREEN}ok${RESET} install directory: $INSTALL_DIR"

cd "$(dirname "$0")"

echo ""
echo "  Building notez (release)..."
QUIET_FLAG=""
if [ ! -t 1 ]; then
    QUIET_FLAG="--quiet"
fi
BUILD_START=$SECONDS
# Unquoted on purpose: empty on a TTY so cargo shows its progress bar.
# shellcheck disable=SC2086
cargo build --release $QUIET_FLAG -p notez-cli
echo "  ${GREEN}ok${RESET} built in $((SECONDS - BUILD_START))s"

mkdir -p "$INSTALL_DIR"
cp target/release/notez "$INSTALL_DIR/notez"
chmod +x "$INSTALL_DIR/notez"
echo "  ${GREEN}ok${RESET} copied to $INSTALL_DIR/notez"

# Not optional on macOS ARM: `cp` over an existing Mach-O invalidates the
# ad-hoc signature and the kernel SIGKILLs the binary on launch. Harmless
# no-op elsewhere.
if command -v codesign &>/dev/null; then
    codesign --force --sign - "$INSTALL_DIR/notez" 2>/dev/null || true
    echo "  ${GREEN}ok${RESET} signed"
else
    echo "  ${GREEN}ok${RESET} skipped (no codesign)"
fi

# Aliases resolved by argv-0 dispatch in crates/notez-cli/src/main.rs.
# Keep this list in sync with `alias_command` there.
#   z<verb>  write/append commands   (zlog, znote, editz)
#   <noun>z  view/manage TUIs        (todoz, logz, zlogs, treez, findz)
ALIASES="todoz zlog logz zlogs znote treez editz findz"
for cmd in $ALIASES; do
    ln -sf "$INSTALL_DIR/notez" "$INSTALL_DIR/$cmd"
done

echo ""
echo "  ${GREEN}ok${RESET} installed $INSTALL_DIR/notez"
echo "  ${GREEN}ok${RESET} aliases: $ALIASES"
"$INSTALL_DIR/notez" --help 2>/dev/null | sed -n 3p

if ! echo "$PATH" | tr ':' '\n' | grep -q "^$INSTALL_DIR$"; then
    echo ""
    echo "  ${PEACH}!${RESET} $INSTALL_DIR is not in your PATH"
    echo "    export PATH=\"$INSTALL_DIR:\$PATH\""
fi

echo ""
echo "  Tab completions (zsh):"
echo "    mkdir -p ~/.zfunc && notez completions zsh > ~/.zfunc/_notez"
echo ""
echo "  ${GREEN}ok${RESET} done in ${SECONDS}s"
echo ""
