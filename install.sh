#!/usr/bin/env bash
# Lilith-TTS install script
# Installs binaries, icons, desktop file, systemd service, and configures permissions.
set -euo pipefail

BOLD='\033[1m'
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info()  { echo -e "${GREEN}[✓]${NC} $*"; }
warn()  { echo -e "${YELLOW}[!]${NC} $*"; }
error() { echo -e "${RED}[✗]${NC} $*"; exit 1; }
step()  { echo -e "\n${BOLD}→ $*${NC}"; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${HOME}/.local/bin"
ICON_DIR="${HOME}/.local/share/icons/hicolor"
DESKTOP_DIR="${HOME}/.local/share/applications"
AUTOSTART_DIR="${HOME}/.config/autostart"
SYSTEMD_DIR="${HOME}/.config/systemd/user"
MODELS_DIR="/var/lib/lilith/models"

# ── Build ────────────────────────────────────────────────────────────────────

step "Building Lilith-TTS (release mode)..."
cd "$SCRIPT_DIR"

# Check for build/runtime dependencies
for dep in python3 python3-pyatspi espeak-ng; do
    if ! dpkg -l "$dep" &>/dev/null; then
        warn "Missing package: $dep. Screen reading or fallback TTS may not work."
        echo "   Try: sudo apt install -y $dep"
    fi
done

# Check for audio players
if ! command -v paplay &>/dev/null && ! command -v aplay &>/dev/null; then
    warn "No audio player (paplay or aplay) found. Audio playback will fail."
    echo "   Try: sudo apt install -y pulseaudio-utils alsa-utils"
fi

cargo build --release 2>&1 | tail -5
info "Build complete"

# ── Install binaries ─────────────────────────────────────────────────────────

step "Installing binaries to $BIN_DIR ..."
mkdir -p "$BIN_DIR"
cp target/release/lilith-tts        "$BIN_DIR/lilith-tts"
cp target/release/lilith-tts-daemon "$BIN_DIR/lilith-tts-daemon"
chmod +x "$BIN_DIR/lilith-tts" "$BIN_DIR/lilith-tts-daemon"
info "Binaries installed"

# ── Install icon ─────────────────────────────────────────────────────────────

step "Installing icon..."
for size in 16 32 48 64 128 256; do
    ICON_SIZE_DIR="$ICON_DIR/${size}x${size}/apps"
    mkdir -p "$ICON_SIZE_DIR"
    if [ -f "$SCRIPT_DIR/assets/icon-${size}.png" ]; then
        cp "$SCRIPT_DIR/assets/icon-${size}.png" "$ICON_SIZE_DIR/lilith-tts.png"
    elif [ -f "$SCRIPT_DIR/assets/icon.png" ]; then
        cp "$SCRIPT_DIR/assets/icon.png" "$ICON_SIZE_DIR/lilith-tts.png"
    fi
done
# Update icon cache if gtk-update-icon-cache is available
if command -v gtk-update-icon-cache &>/dev/null; then
    gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true
fi
info "Icon installed"

# ── Desktop/autostart entries ─────────────────────────────────────────────────

step "Installing desktop entry and autostart..."
mkdir -p "$DESKTOP_DIR" "$AUTOSTART_DIR"
sed "s|Exec=lilith-tts|Exec=$BIN_DIR/lilith-tts|g" \
    "$SCRIPT_DIR/assets/lilith-tts.desktop" > "$DESKTOP_DIR/lilith-tts.desktop"
cp "$DESKTOP_DIR/lilith-tts.desktop" "$AUTOSTART_DIR/lilith-tts.desktop"
info "Desktop entries installed"

# ── Systemd user service ──────────────────────────────────────────────────────

step "Installing systemd user service for daemon..."
mkdir -p "$SYSTEMD_DIR"
sed "s|%h|$HOME|g" \
    "$SCRIPT_DIR/assets/lilith-tts-daemon.service" \
    > "$SYSTEMD_DIR/lilith-tts-daemon.service"
systemctl --user daemon-reload
systemctl --user enable lilith-tts-daemon.service
info "Daemon service enabled"

# ── Input group for hotkey access ─────────────────────────────────────────────

step "Checking input group membership for Ctrl+T+T+M hotkey..."
if groups "$USER" | grep -q '\binput\b'; then
    info "User is already in the 'input' group"
else
    warn "User is NOT in the 'input' group — hotkey will not work until fixed."
    echo "   Run the following command and log out/in:"
    echo -e "   ${BOLD}sudo usermod -aG input $USER${NC}"
fi

# ── Model directory ───────────────────────────────────────────────────────────

step "Checking NeuTTS model..."
if [ -f "$MODELS_DIR/neutts-nano-q4.gguf" ]; then
    info "NeuTTS Nano model found at $MODELS_DIR/neutts-nano-q4.gguf"
else
    warn "NeuTTS model not found at $MODELS_DIR/neutts-nano-q4.gguf"
    echo "   Lilith-TTS will fall back to espeak-ng until the model is placed there."
    echo "   Download: huggingface-cli download neuphonic/neutts-nano-q4 --local-dir $MODELS_DIR"
fi

# ── PATH check ────────────────────────────────────────────────────────────────

if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    warn "$BIN_DIR is not in PATH. Add to your ~/.bashrc or ~/.profile:"
    echo "   export PATH=\"\$PATH:$BIN_DIR\""
fi

# ── Done ──────────────────────────────────────────────────────────────────────

echo ""
info "${BOLD}Lilith-TTS installed successfully!${NC}"
echo ""
echo "  Start daemon:   systemctl --user start lilith-tts-daemon"
echo "  Open GUI:       lilith-tts"
echo "  Hotkey:         Ctrl + T + T + M  (from any application)"
echo "  Config:         ~/.config/lilith-tts/config.toml"
echo ""
