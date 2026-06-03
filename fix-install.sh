#!/usr/bin/env bash
# ╔══════════════════════════════════════════════════════════════════╗
# ║  Lilith-TTS — System Install Fix                                 ║
# ║  Installs binary to /usr/local/bin and desktop to               ║
# ║  /usr/share/applications — matching how COSMIC discovers        ║
# ║  third-party applets like cosmic-ext-applet-caffeine            ║
# ╚══════════════════════════════════════════════════════════════════╝
set -euo pipefail

BOLD='\033[1m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
info()  { echo -e "${GREEN}[✓]${NC} $*"; }
warn()  { echo -e "${YELLOW}[!]${NC} $*"; }
step()  { echo -e "\n${BOLD}${CYAN}▶ $*${NC}"; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_ID="io.lilith.LilithTts"
BIN_NAME="lilith-tts"
DAEMON_NAME="lilith-tts-daemon"

# ── 1. Verify new binary exists ─────────────────────────────────────────────

step "Checking build..."
if [[ ! -f "$SCRIPT_DIR/target/release/$BIN_NAME" ]]; then
    echo "Binary not found. Building now..."
    cd "$SCRIPT_DIR"
    cargo build --release
fi

# Show build date
BIN_DATE=$(stat --format="%y" "$SCRIPT_DIR/target/release/$BIN_NAME")
info "Binary date: $BIN_DATE"

# ── 2. Install binary to /usr/local/bin (on PATH, no full path needed) ──────

step "Installing binary to /usr/local/bin (requires sudo)..."
sudo install -m 0755 "$SCRIPT_DIR/target/release/$BIN_NAME"     /usr/local/bin/$BIN_NAME
sudo install -m 0755 "$SCRIPT_DIR/target/release/$DAEMON_NAME"  /usr/local/bin/$DAEMON_NAME
info "Binaries installed to /usr/local/bin/"

# Also keep ~/.local/bin copy for non-panel use
mkdir -p "$HOME/.local/bin"
install -m 0755 "$SCRIPT_DIR/target/release/$BIN_NAME"    "$HOME/.local/bin/$BIN_NAME"
install -m 0755 "$SCRIPT_DIR/target/release/$DAEMON_NAME" "$HOME/.local/bin/$DAEMON_NAME"

# ── 3. Install icons to /usr/share/icons (system-wide, so COSMIC finds them) ─

step "Installing icons (requires sudo)..."
SVG_DIR="/usr/share/icons/hicolor/scalable/apps"
sudo mkdir -p "$SVG_DIR"
sudo install -m 0644 "$SCRIPT_DIR/assets/${APP_ID}.svg"          "$SVG_DIR/${APP_ID}.svg"
sudo install -m 0644 "$SCRIPT_DIR/assets/${APP_ID}-symbolic.svg" "$SVG_DIR/${APP_ID}-symbolic.svg"
info "SVG icons installed to $SVG_DIR"

# Refresh icon cache
sudo gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true

# ── 4. Write the correct .desktop file to /usr/share/applications ───────────
#
# Key requirements discovered from installed applets like caffeine/battery:
#  - Exec= uses just the binary name (must be on PATH)
#  - No %F or %U arguments
#  - Icon= uses the -symbolic variant name
#  - X-CosmicApplet=true  (required for "Add Applet" listing)
#  - NoDisplay=true       (hide from app launcher, show in panel settings only)
#  - X-CosmicHoverPopup=Auto (popup anchors to the panel button)

step "Installing .desktop file to /usr/share/applications (requires sudo)..."
sudo tee /usr/share/applications/${APP_ID}.desktop > /dev/null <<'DESKTOP'
[Desktop Entry]
Name=Lilith TTS
GenericName=Text to Speech
Comment=Lilith Linux system-wide TTS panel applet
Type=Application
Exec=lilith-tts
Terminal=false
StartupNotify=true
NoDisplay=true
Categories=COSMIC;Accessibility;Utility;
Keywords=COSMIC;TTS;Text;Speech;Screen;Reader;Accessibility;
Icon=io.lilith.LilithTts-symbolic
X-CosmicApplet=true
X-CosmicHoverPopup=Auto
DESKTOP

info ".desktop written to /usr/share/applications/${APP_ID}.desktop"

# Update desktop database
sudo update-desktop-database /usr/share/applications 2>/dev/null || true

# Also remove the old incorrect user-local .desktop if it exists
if [[ -f "$HOME/.local/share/applications/${APP_ID}.desktop" ]]; then
    rm -f "$HOME/.local/share/applications/${APP_ID}.desktop"
    warn "Removed old ~/.local/share/applications/${APP_ID}.desktop (was using full path + %F)"
    update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
fi

# ── 5. Daemon systemd service ────────────────────────────────────────────────

step "Installing systemd user service..."
SYSTEMD_DIR="$HOME/.config/systemd/user"
mkdir -p "$SYSTEMD_DIR"
cat > "$SYSTEMD_DIR/lilith-tts-daemon.service" <<EOF
[Unit]
Description=Lilith TTS Background Daemon
Documentation=https://github.com/BlancoBAM/Lilith-TTS
After=graphical-session.target
PartOf=graphical-session.target

[Service]
Type=simple
ExecStart=/usr/local/bin/lilith-tts-daemon
Restart=on-failure
RestartSec=5s
Environment=RUST_LOG=tts_daemon=info,tts_core=info

[Install]
WantedBy=graphical-session.target
EOF

systemctl --user daemon-reload
systemctl --user enable lilith-tts-daemon.service
systemctl --user restart lilith-tts-daemon.service 2>/dev/null || systemctl --user start lilith-tts-daemon.service

if systemctl --user is-active --quiet lilith-tts-daemon.service; then
    info "Daemon service is running"
else
    warn "Daemon not started yet (may need login session)"
fi

# ── 6. Input group for hotkey ────────────────────────────────────────────────

step "Checking input group for Ctrl+T+T+M hotkey..."
if groups "$USER" | grep -qw 'input'; then
    info "Already in 'input' group — hotkey ready"
else
    warn "Not in 'input' group. Adding (requires sudo)..."
    sudo usermod -aG input "$USER"
    info "Added to 'input' group — log out/in for hotkey to activate"
fi

# ── 7. Done ──────────────────────────────────────────────────────────────────

echo ""
echo -e "${GREEN}${BOLD}╔══════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}${BOLD}║  Lilith TTS install complete!                    ║${NC}"
echo -e "${GREEN}${BOLD}╚══════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "  ${BOLD}Add to COSMIC Panel:${NC}"
echo    "    System Settings → Panels → Panel → Add Applet → Lilith TTS"
echo    "    (Or right-click the panel → Manage Panel Applets)"
echo ""
echo -e "  ${BOLD}Daemon control:${NC}"
echo    "    systemctl --user status  lilith-tts-daemon"
echo    "    systemctl --user restart lilith-tts-daemon"
echo ""
echo -e "  ${BOLD}Hotkey:${NC}  Hold Ctrl, then press T T M  (from any app)"
echo ""
if ! groups "$USER" | grep -qw 'input'; then
    echo -e "  ${YELLOW}${BOLD}⚠  Log out and back in for the Ctrl+T+T+M hotkey to activate${NC}"
    echo ""
fi
echo -e "  ${BOLD}If the applet shows a generic icon after adding it:${NC}"
echo    "    Log out and back in to reload the icon theme cache"
echo ""
