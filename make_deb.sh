#!/usr/bin/env bash
# Lilith-TTS .deb package builder
set -euo pipefail

BOLD='\033[1m'
GREEN='\033[0;32m'
NC='\033[0m'

info() { echo -e "${GREEN}[✓]${NC} $*"; }

APP_NAME="lilith-tts"
VERSION="0.1.0"
ARCH=$(dpkg --print-architecture)
PKG_DIR="target/debian/${APP_NAME}_${VERSION}_${ARCH}"

info "Building Lilith-TTS in release mode..."
cargo build --release

info "Preparing package structure in $PKG_DIR..."
rm -rf "$PKG_DIR"
mkdir -p "$PKG_DIR/DEBIAN"
mkdir -p "$PKG_DIR/usr/bin"
mkdir -p "$PKG_DIR/usr/share/applications"
mkdir -p "$PKG_DIR/usr/share/icons/hicolor/scalable/apps"
mkdir -p "$PKG_DIR/usr/lib/systemd/user"
mkdir -p "$PKG_DIR/etc/xdg/autostart"

# 1. Binaries
cp target/release/lilith-tts        "$PKG_DIR/usr/bin/"
cp target/release/lilith-tts-daemon "$PKG_DIR/usr/bin/"

# 2. Desktop file
cp assets/lilith-tts.desktop "$PKG_DIR/usr/share/applications/"
cp assets/lilith-tts.desktop "$PKG_DIR/etc/xdg/autostart/"

# 3. Icon (Assuming icon.png is the source)
# For a proper deb, we should ideally have multiple sizes, but we'll put it in scalable for now
cp assets/icon.png "$PKG_DIR/usr/share/icons/hicolor/scalable/apps/lilith-tts.png"

# 4. Systemd service
cp assets/lilith-tts-daemon.service "$PKG_DIR/usr/lib/systemd/user/"

# 5. Control file
cat > "$PKG_DIR/DEBIAN/control" <<EOF
Package: $APP_NAME
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Depends: python3, python3-pyatspi, espeak-ng, pulseaudio-utils | alsa-utils
Maintainer: BlancoBAM <admin@lilith-linux.org>
Description: System-wide local TTS for Lilith Linux.
 High-performance local text-to-speech with NeuTTS Nano, 
 global hotkey support, and intelligent screen reading.
EOF

# 6. Post-install script (to reload systemd and update icon cache)
cat > "$PKG_DIR/DEBIAN/postinst" <<EOF
#!/bin/sh
set -e
if [ "\$1" = "configure" ]; then
    gtk-update-icon-cache -f -t /usr/share/icons/hicolor || true
    # Note: user systemd services don't need root reload here, 
    # but we can trigger a global one just in case.
fi
EOF
chmod 755 "$PKG_DIR/DEBIAN/postinst"

info "Building .deb package..."
dpkg-deb --build "$PKG_DIR"

info "Package created: ${PKG_DIR}.deb"
info "Install with: sudo dpkg -i ${PKG_DIR}.deb"
