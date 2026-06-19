#!/usr/bin/env bash
# ╔══════════════════════════════════════════════════════════════════╗
# ║  Lilith-TTS — One-Shot Install Script                           ║
# ║  Builds, installs binaries, icons, desktop entry, daemon        ║
# ║  service, adds user to input group for global hotkey, and       ║
# ║  optionally downloads the NeuTTS Nano model.                    ║
# ╚══════════════════════════════════════════════════════════════════╝
set -euo pipefail

BOLD='\033[1m'
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${GREEN}[✓]${NC} $*"; }
warn()  { echo -e "${YELLOW}[!]${NC} $*"; }
error() { echo -e "${RED}[✗]${NC} $*"; exit 1; }
step()  { echo -e "\n${BOLD}${CYAN}▶ $*${NC}"; }
note()  { echo -e "  ${CYAN}$*${NC}"; }

# ── Paths ────────────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${HOME}/.local/bin"
ICON_DIR="${HOME}/.local/share/icons/hicolor"
DESKTOP_DIR="${HOME}/.local/share/applications"
AUTOSTART_DIR="${HOME}/.config/autostart"
SYSTEMD_DIR="${HOME}/.config/systemd/user"
APP_ID="io.lilith.LilithTts"
NEUTTS_MODEL_DIR="/var/lib/lilith/models"
NEUTTS_MODEL_FILE="${NEUTTS_MODEL_DIR}/neutts-nano-q4.gguf"
NEUTTS_MODEL_LOCAL="${HOME}/.local/share/lilith-tts/models/neutts-nano-q4.gguf"
HF_REPO="neuphonic/neutts-nano-q4"
HF_FILENAME="neutts-nano-q4.gguf"
HF_DIRECT_URL="https://huggingface.co/neuphonic/neutts-nano-q4/resolve/main/neutts-nano-q4.gguf"

# ── Options Parsing ──────────────────────────────────────────────────────────

ONLY_MODEL=false
NON_INTERACTIVE=false

show_help() {
    echo "Usage: ./install.sh [options]"
    echo ""
    echo "Options:"
    echo "  --only-model       Only check and download the NeuTTS Nano GGUF model"
    echo "  --non-interactive  Run installation without prompting (use defaults)"
    echo "  -h, --help         Show this help message"
    echo ""
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --only-model)
            ONLY_MODEL=true
            shift
            ;;
        --non-interactive)
            NON_INTERACTIVE=true
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            echo "Unknown argument: $1"
            show_help
            exit 1
            ;;
    esac
done

if [[ "$ONLY_MODEL" == "false" ]]; then

# ── Check build requirements ─────────────────────────────────────────────────

step "Checking build environment..."
if ! command -v cargo &>/dev/null; then
    error "Rust/Cargo not found. Install from https://rustup.rs"
fi
info "Rust $(rustc --version | cut -d' ' -f2) found"

# Check runtime/accessibility dependencies
for dep in python3 espeak-ng; do
    if command -v "$dep" &>/dev/null; then
        info "Found: $dep"
    else
        warn "Missing: $dep — screen reading or TTS fallback may be limited"
        note "Install: sudo apt install -y $dep"
    fi
done

# Check AT-SPI Python bindings
if python3 -c "import pyatspi" 2>/dev/null; then
    info "Found: python3-pyatspi (AT-SPI screen reading)"
else
    warn "python3-pyatspi not installed — screen reading will be unavailable"
    note "Install: sudo apt install -y python3-pyatspi"
fi

# Audio player check
if command -v paplay &>/dev/null; then
    info "Found audio player: paplay (PipeWire/PulseAudio)"
elif command -v aplay &>/dev/null; then
    info "Found audio player: aplay (ALSA)"
else
    warn "No audio player found (paplay or aplay)"
    note "Install: sudo apt install -y pulseaudio-utils alsa-utils"
fi

# ── Install C build dependencies ──────────────────────────────────────────────

step "Installing C build dependencies..."
BUILD_DEPS=(pkg-config libxkbcommon-dev libwayland-dev libdbus-1-dev libasound2-dev)
MISSING_DEPS=()
for dep in "${BUILD_DEPS[@]}"; do
    if ! dpkg -s "$dep" &>/dev/null 2>&1; then
        MISSING_DEPS+=("$dep")
    fi
done
if [ ${#MISSING_DEPS[@]} -gt 0 ]; then
    note "Installing missing build dependencies: ${MISSING_DEPS[*]}"
    sudo apt-get install -y "${MISSING_DEPS[@]}" || error "Failed to install build dependencies. Run: sudo apt install -y ${MISSING_DEPS[*]}"
    info "Build dependencies installed"
else
    info "All C build dependencies present"
fi

# ── Build ────────────────────────────────────────────────────────────────────

step "Building Lilith-TTS (release)..."
cd "$SCRIPT_DIR"
cargo build --release 2>&1 | grep -E "^(error|warning\[|Compiling|Finished)" | tail -20 || true
if [[ ! -f "target/release/lilith-tts" || ! -f "target/release/lilith-tts-daemon" ]]; then
    error "Build failed — binaries not found in target/release/"
fi
info "Build complete"

# ── Install binaries ─────────────────────────────────────────────────────────

step "Installing binaries to ${BIN_DIR}..."
mkdir -p "$BIN_DIR"
install -m 0755 target/release/lilith-tts        "$BIN_DIR/lilith-tts"
install -m 0755 target/release/lilith-tts-daemon "$BIN_DIR/lilith-tts-daemon"
info "Binaries installed"

# Ensure ~/.local/bin is on PATH
if [[ ":$PATH:" != *":${BIN_DIR}:"* ]]; then
    warn "${BIN_DIR} is not in PATH"
    note "Add to ~/.bashrc or ~/.profile:"
    note "  export PATH=\"\$PATH:${BIN_DIR}\""
    # Try to add it automatically to ~/.profile if it doesn't already include it
    if ! grep -q "\.local/bin" "${HOME}/.profile" 2>/dev/null; then
        echo '' >> "${HOME}/.profile"
        echo '# Added by Lilith-TTS installer' >> "${HOME}/.profile"
        echo 'export PATH="$PATH:$HOME/.local/bin"' >> "${HOME}/.profile"
        info "Added ${BIN_DIR} to ~/.profile (effective on next login)"
    fi
fi

# ── Install SVG icon ─────────────────────────────────────────────────────────

step "Installing icon..."
# Scalable SVG — preferred by all icon themes, perfect on HiDPI
SVG_DEST="${ICON_DIR}/scalable/apps/${APP_ID}.svg"
mkdir -p "$(dirname "$SVG_DEST")"
if [[ -f "$SCRIPT_DIR/assets/${APP_ID}.svg" ]]; then
    install -m 0644 "$SCRIPT_DIR/assets/${APP_ID}.svg" "$SVG_DEST"
    info "Scalable SVG icon installed"
else
    warn "SVG icon not found at assets/${APP_ID}.svg"
fi

# Symbolic icon (monochrome, used on the panel button itself)
SYM_DEST="${ICON_DIR}/scalable/apps/${APP_ID}-symbolic.svg"
if [[ -f "$SCRIPT_DIR/assets/${APP_ID}-symbolic.svg" ]]; then
    install -m 0644 "$SCRIPT_DIR/assets/${APP_ID}-symbolic.svg" "$SYM_DEST"
    info "Symbolic panel icon installed"
fi

# Also install any PNG sizes if present (fallback for apps that can't use SVG)
for size in 16 32 48 64 128 256; do
    PNG_SIZE_DIR="${ICON_DIR}/${size}x${size}/apps"
    mkdir -p "$PNG_SIZE_DIR"
    if [[ -f "$SCRIPT_DIR/assets/icon-${size}.png" ]]; then
        install -m 0644 "$SCRIPT_DIR/assets/icon-${size}.png" \
            "${PNG_SIZE_DIR}/${APP_ID}.png"
    elif [[ -f "$SCRIPT_DIR/assets/icon.png" ]]; then
        install -m 0644 "$SCRIPT_DIR/assets/icon.png" \
            "${PNG_SIZE_DIR}/${APP_ID}.png"
    fi
done

# Refresh icon cache
if command -v gtk-update-icon-cache &>/dev/null; then
    gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true
fi
info "Icons installed"

# ── Desktop entry ─────────────────────────────────────────────────────────────

step "Installing desktop entry..."
mkdir -p "$DESKTOP_DIR"
sed "s|Exec=lilith-tts|Exec=${BIN_DIR}/lilith-tts|g" \
    "$SCRIPT_DIR/assets/lilith-tts.desktop" > "$DESKTOP_DIR/${APP_ID}.desktop"
info "Desktop entry installed: ${DESKTOP_DIR}/${APP_ID}.desktop"

# Update desktop database so COSMIC Panel can find the applet
if command -v update-desktop-database &>/dev/null; then
    update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
fi

# ── Autostart entry (also starts GUI applet on session login) ─────────────────

step "Installing autostart entry..."
mkdir -p "$AUTOSTART_DIR"
# The applet is launched by COSMIC Panel directly when the user adds it to the panel.
# We also provide an autostart entry that keeps the daemon running on session start.
cat > "$AUTOSTART_DIR/lilith-tts-daemon.desktop" <<EOF
[Desktop Entry]
Name=Lilith TTS Daemon
Comment=Background daemon for Lilith TTS global hotkey and TTS engine
Type=Application
Exec=${BIN_DIR}/lilith-tts-daemon
Hidden=false
NoDisplay=true
X-GNOME-Autostart-enabled=true
X-COSMIC-Autostart=true
EOF
info "Daemon autostart entry installed"

# ── Systemd user service ──────────────────────────────────────────────────────

step "Installing systemd user service for daemon..."
mkdir -p "$SYSTEMD_DIR"
sed "s|%h|${HOME}|g" \
    "$SCRIPT_DIR/assets/lilith-tts-daemon.service" \
    > "$SYSTEMD_DIR/lilith-tts-daemon.service"

# Patch ExecStart to point to the installed binary
sed -i "s|ExecStart=.*|ExecStart=${BIN_DIR}/lilith-tts-daemon|g" \
    "$SYSTEMD_DIR/lilith-tts-daemon.service"

systemctl --user daemon-reload
systemctl --user enable --now lilith-tts-daemon.service

if systemctl --user is-active --quiet lilith-tts-daemon.service; then
    info "Daemon service is running"
else
    warn "Daemon service did not start automatically"
    note "Start manually: systemctl --user start lilith-tts-daemon"
fi

# ── Global hotkey: input group membership ─────────────────────────────────────

step "Setting up global hotkey (Ctrl + T + T + M)..."
note "The daemon listens to /dev/input/* directly — works on X11 and Wayland."
note "This requires your user account to be in the 'input' group."

if groups "$USER" | grep -qw 'input'; then
    info "User '${USER}' is already in the 'input' group — hotkey is ready"
else
    warn "User '${USER}' is NOT in the 'input' group"
    note "Adding you to the input group now (requires sudo)..."
    echo ""
    if sudo usermod -aG input "$USER"; then
        info "Added '${USER}' to the 'input' group"
        note ""
        note "${BOLD}${YELLOW}IMPORTANT:${NC} You must log out and back in (or reboot) for the"
        note "group change to take effect. After that, the Ctrl+T+T+M hotkey"
        note "will work from any application, including Wayland-native apps."
        note ""
        note "To apply immediately without logging out (current shell only):"
        note "  newgrp input"
        # Also restart the daemon so it picks up the new group in this session
        note ""
        note "Then restart the daemon:"
        note "  systemctl --user restart lilith-tts-daemon"
    else
        warn "Could not add to input group automatically"
        note "Run this command manually:"
        note "  sudo usermod -aG input ${USER}"
        note "Then log out and back in."
    fi
fi

fi # Close ONLY_MODEL conditional

# ── NeuTTS Nano model ─────────────────────────────────────────────────────────

step "Checking NeuTTS Nano model..."

# Check system location first, then user-local fallback
if [[ -f "$NEUTTS_MODEL_FILE" ]]; then
    info "NeuTTS model found at ${NEUTTS_MODEL_FILE}"
    MODEL_FOUND=true
elif [[ -f "$NEUTTS_MODEL_LOCAL" ]]; then
    info "NeuTTS model found at ${NEUTTS_MODEL_LOCAL} (user-local)"
    MODEL_FOUND=true
else
    MODEL_FOUND=false
    warn "NeuTTS Nano model not found"
    echo ""
    echo -e "  ${BOLD}The NeuTTS Nano model (~120MB) is required for high-quality TTS.${NC}"
    echo "  Without it, the app falls back to espeak-ng."
    echo ""
fi

if [[ "$MODEL_FOUND" == "false" ]]; then
    if [[ "$NON_INTERACTIVE" == "true" ]]; then
        info "Non-interactive installation: downloading model to user-local destination..."
        model_choice=2
    else
        echo -e "  ${BOLD}Would you like to download the model now?${NC}"
        echo "  [1] Yes — download to ${NEUTTS_MODEL_FILE} (recommended, requires sudo)"
        echo "  [2] Yes — download to ${NEUTTS_MODEL_LOCAL} (user-local, no sudo)"
        echo "  [3] No  — I'll add the model path manually in the app settings"
        echo ""
        read -rp "  Choice [1/2/3]: " model_choice
    fi

    case "${model_choice:-1}" in
        1)
            step "Downloading NeuTTS Nano model (system-wide)..."
            if ! sudo mkdir -p "$NEUTTS_MODEL_DIR"; then
                warn "Could not create ${NEUTTS_MODEL_DIR} — falling back to user-local"
                model_choice=2
            fi
            ;;
    esac

    case "${model_choice:-1}" in
        1)
            DOWNLOAD_DEST="$NEUTTS_MODEL_FILE"
            SUDO_PREFIX="sudo"
            ;;
        2)
            mkdir -p "$(dirname "$NEUTTS_MODEL_LOCAL")"
            DOWNLOAD_DEST="$NEUTTS_MODEL_LOCAL"
            SUDO_PREFIX=""
            ;;
        3)
            info "Skipping model download. Set the model path in the app's Settings panel."
            DOWNLOAD_DEST=""
            ;;
        *)
            DOWNLOAD_DEST=""
            ;;
    esac

    if [[ -n "$DOWNLOAD_DEST" ]]; then
        # Try huggingface-cli first (supports auth + resuming)
        if command -v huggingface-cli &>/dev/null; then
            note "Using huggingface-cli…"
            if $SUDO_PREFIX huggingface-cli download "$HF_REPO" "$HF_FILENAME" \
                --local-dir "$(dirname "$DOWNLOAD_DEST")"; then
                info "Model downloaded to $(dirname "$DOWNLOAD_DEST")"
            else
                warn "huggingface-cli download failed — trying curl"
                model_choice="curl"
            fi
        else
            model_choice="curl"
        fi

        if [[ "${model_choice}" == "curl" ]] || ! command -v huggingface-cli &>/dev/null; then
            note "Downloading via curl (${HF_DIRECT_URL})…"
            if $SUDO_PREFIX curl -L --progress-bar \
                -o "$DOWNLOAD_DEST" \
                "$HF_DIRECT_URL"; then
                info "Model downloaded to ${DOWNLOAD_DEST}"
            else
                warn "Download failed. You can download it manually:"
                note "  curl -L -o '${DOWNLOAD_DEST}' '${HF_DIRECT_URL}'"
                note "Or configure a different model in the Settings panel."
            fi
        fi

        # Update config to point to downloaded model location
        CONFIG_FILE="${HOME}/.config/lilith-tts/config.toml"
        if [[ -f "$CONFIG_FILE" ]]; then
            sed -i "s|model_path = .*|model_path = \"${DOWNLOAD_DEST}\"|g" "$CONFIG_FILE" || true
            info "Updated config to use downloaded model"
        fi
    fi
fi

# ── Done ──────────────────────────────────────────────────────────────────────

echo ""
echo -e "${GREEN}${BOLD}╔════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}${BOLD}║   Lilith TTS installed successfully!        ║${NC}"
echo -e "${GREEN}${BOLD}╚════════════════════════════════════════════╝${NC}"
echo ""
echo -e "  ${BOLD}Add to COSMIC Panel:${NC}"
echo    "    Right-click panel → Add Applet → Lilith TTS"
echo ""
echo -e "  ${BOLD}Daemon control:${NC}"
echo    "    systemctl --user status  lilith-tts-daemon"
echo    "    systemctl --user restart lilith-tts-daemon"
echo ""
echo -e "  ${BOLD}Global hotkey:${NC}  Ctrl + T + T + M (from any application)"
echo -e "  ${BOLD}Config file:${NC}    ~/.config/lilith-tts/config.toml"
echo ""
if ! groups "$USER" | grep -qw 'input'; then
    echo -e "  ${YELLOW}${BOLD}Remember:${NC} Log out and back in for the hotkey to activate."
    echo ""
fi
