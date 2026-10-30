#!/bin/sh
# Installs Colour Picker for the current user. No sudo: everything goes under your home folder.
#   ./install.sh                                   binary to ~/.local/bin
#   PREFIX=~/Applications/ColourPicker ./install.sh  binary to a folder of your choice
#   ./install.sh --uninstall                       removes what this script added
# Run it from the release folder (next to the `colourpicker` binary) or after `make` (uses bin/).
set -eu
cd "$(dirname "$0")"
if [ "$(id -u)" -eq 0 ]; then
    echo "Run this as your normal user, not as root." >&2
    exit 1
fi
ID=io.github.droidstreamer.ColourPicker
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
DESKTOP="$DATA/applications/$ID.desktop"
ICON="$DATA/icons/hicolor/256x256/apps/$ID.png"

if [ "${1:-}" = "--uninstall" ]; then
    BIN=$(sed -n 's/^Exec=//p' "$DESKTOP" 2>/dev/null || true)
    rm -f "$DESKTOP" "$ICON"
    [ -n "$BIN" ] && rm -f "$BIN"
    echo "Removed Colour Picker."
    exit 0
fi

PREFIX="${PREFIX:-$HOME/.local/bin}"
case "$PREFIX" in
    /*) ;;
    *) echo "PREFIX must be an absolute path, got: $PREFIX" >&2; exit 1 ;;
esac
case "$PREFIX" in
    *[[:space:]]*) echo "PREFIX must not contain spaces: $PREFIX" >&2; exit 1 ;;
esac
SRC=colourpicker
[ -f "$SRC" ] || SRC=bin/colourpicker
[ -f colourpicker.png ] && PNG=colourpicker.png || PNG=../Icons/colourpicker.png
BIN="$PREFIX/colourpicker"

install -Dm755 "$SRC" "$BIN"
install -Dm644 "$PNG" "$ICON"
mkdir -p "$(dirname "$DESKTOP")"
cat > "$DESKTOP" <<EOF
[Desktop Entry]
Type=Application
Name=Colour Picker
Comment=Pick any colour on screen and see its name
Exec=$BIN
Icon=$ID
Categories=Graphics;Utility;
Keywords=color;colour;picker;eyedropper;hex;
StartupNotify=true
EOF
chmod 644 "$DESKTOP"
update-desktop-database -q "$DATA/applications" 2>/dev/null || true

echo "Installed Colour Picker:"
echo "  $BIN"
echo "  $DESKTOP"
echo "  $ICON"
echo "Find it in Activities. Remove it with ./install.sh --uninstall"
