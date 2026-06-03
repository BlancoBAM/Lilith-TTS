# Lilith-TTS Justfile — standard COSMIC applet build & install recipes
# Usage:
#   just                      # Build release binaries
#   just install              # Install system-wide (default prefix=/usr)
#   just install-user         # Install to user-local directories (~/.local)
#   just download-model       # Interactively check/download NeuTTS Nano GGUF model
#   just setup-hotkey         # Add user to 'input' group for global hotkey

rootdir := ''
prefix := '/usr'
cargo-target-dir := env('CARGO_TARGET_DIR', 'target')

# Installation paths
base-dir := absolute_path(clean(rootdir / prefix))
bin-dst := base-dir / 'bin'
desktop-dst := base-dir / 'share' / 'applications'
icon-dst-scalable := base-dir / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps'
systemd-user-dst := base-dir / 'lib' / 'systemd' / 'user'

# Default recipe which builds the release profile
default: build-release

# Clean cargo target directory
clean:
    cargo clean

# Build debug binaries
build-debug *args:
    cargo build {{args}}

# Build release binaries
build-release *args: (build-debug '--release' args)

# System-wide installation (typically requires sudo)
# Usage: sudo just install or just install prefix=/usr/local (with sudo)
install: build-release
    install -Dm0755 {{ cargo-target-dir }}/release/lilith-tts {{ bin-dst }}/lilith-tts
    install -Dm0755 {{ cargo-target-dir }}/release/lilith-tts-daemon {{ bin-dst }}/lilith-tts-daemon
    install -Dm0644 assets/io.lilith.LilithTts.svg {{ icon-dst-scalable }}/io.lilith.LilithTts.svg
    install -Dm0644 assets/io.lilith.LilithTts-symbolic.svg {{ icon-dst-scalable }}/io.lilith.LilithTts-symbolic.svg
    
    # Configure and install desktop file
    mkdir -p {{ desktop-dst }}
    install -m 0644 assets/lilith-tts.desktop {{ desktop-dst }}/io.lilith.LilithTts.desktop
    
    # Configure and install systemd user service
    mkdir -p {{ systemd-user-dst }}
    sed "s|%h/.local/bin/lilith-tts-daemon|{{ prefix }}/bin/lilith-tts-daemon|g" \
        assets/lilith-tts-daemon.service > {{ systemd-user-dst }}/lilith-tts-daemon.service
    chmod 0644 {{ systemd-user-dst }}/lilith-tts-daemon.service
    
    # Update system caches if applicable
    if [ -z "{{ rootdir }}" ]; then \
        update-desktop-database {{ desktop-dst }} 2>/dev/null || true; \
        gtk-update-icon-cache -f -t {{ base-dir }}/share/icons/hicolor 2>/dev/null || true; \
    fi

# System-wide uninstallation
uninstall:
    rm -f {{ bin-dst }}/lilith-tts
    rm -f {{ bin-dst }}/lilith-tts-daemon
    rm -f {{ icon-dst-scalable }}/io.lilith.LilithTts.svg
    rm -f {{ icon-dst-scalable }}/io.lilith.LilithTts-symbolic.svg
    rm -f {{ desktop-dst }}/io.lilith.LilithTts.desktop
    rm -f {{ systemd-user-dst }}/lilith-tts-daemon.service
    if [ -z "{{ rootdir }}" ]; then \
        update-desktop-database {{ desktop-dst }} 2>/dev/null || true; \
        gtk-update-icon-cache -f -t {{ base-dir }}/share/icons/hicolor 2>/dev/null || true; \
    fi

# User-local installation (no root privileges required)
install-user: build-release
    # Create target directories
    mkdir -p ~/.local/bin
    mkdir -p ~/.local/share/applications
    mkdir -p ~/.local/share/icons/hicolor/scalable/apps
    mkdir -p ~/.config/systemd/user
    mkdir -p ~/.config/autostart
    
    # Copy binaries
    install -m 0755 {{ cargo-target-dir }}/release/lilith-tts ~/.local/bin/lilith-tts
    install -m 0755 {{ cargo-target-dir }}/release/lilith-tts-daemon ~/.local/bin/lilith-tts-daemon
    
    # Copy icons
    install -m 0644 assets/io.lilith.LilithTts.svg ~/.local/share/icons/hicolor/scalable/apps/io.lilith.LilithTts.svg
    install -m 0644 assets/io.lilith.LilithTts-symbolic.svg ~/.local/share/icons/hicolor/scalable/apps/io.lilith.LilithTts-symbolic.svg
    
    # Install custom desktop file with full path in Exec (so it launches properly in any case)
    sed "s|Exec=lilith-tts|Exec=$HOME/.local/bin/lilith-tts|g" \
        assets/lilith-tts.desktop > ~/.local/share/applications/io.lilith.LilithTts.desktop
    chmod 0644 ~/.local/share/applications/io.lilith.LilithTts.desktop
    
    # Install daemon autostart
    echo "[Desktop Entry]" > ~/.config/autostart/lilith-tts-daemon.desktop
    echo "Name=Lilith TTS Daemon" >> ~/.config/autostart/lilith-tts-daemon.desktop
    echo "Comment=Background daemon for Lilith TTS" >> ~/.config/autostart/lilith-tts-daemon.desktop
    echo "Type=Application" >> ~/.config/autostart/lilith-tts-daemon.desktop
    echo "Exec=$HOME/.local/bin/lilith-tts-daemon" >> ~/.config/autostart/lilith-tts-daemon.desktop
    echo "Hidden=false" >> ~/.config/autostart/lilith-tts-daemon.desktop
    echo "NoDisplay=true" >> ~/.config/autostart/lilith-tts-daemon.desktop
    echo "X-COSMIC-Autostart=true" >> ~/.config/autostart/lilith-tts-daemon.desktop
    
    # Install systemd user service pointing to local path
    sed "s|%h|$HOME|g" assets/lilith-tts-daemon.service > ~/.config/systemd/user/lilith-tts-daemon.service
    chmod 0644 ~/.config/systemd/user/lilith-tts-daemon.service
    
    # Update databases
    update-desktop-database ~/.local/share/applications 2>/dev/null || true
    gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor 2>/dev/null || true
    
    # Enable and start user service
    systemctl --user daemon-reload
    systemctl --user enable --now lilith-tts-daemon.service
    
    @echo ""
    @echo "=================================================="
    @echo "Lilith TTS (User-local) installed successfully!"
    @echo "Add the applet from your COSMIC settings/panel."
    @echo "=================================================="

# Uninstall user-local files
uninstall-user:
    rm -f ~/.local/bin/lilith-tts
    rm -f ~/.local/bin/lilith-tts-daemon
    rm -f ~/.local/share/icons/hicolor/scalable/apps/io.lilith.LilithTts.svg
    rm -f ~/.local/share/icons/hicolor/scalable/apps/io.lilith.LilithTts-symbolic.svg
    rm -f ~/.local/share/applications/io.lilith.LilithTts.desktop
    rm -f ~/.config/autostart/lilith-tts-daemon.desktop
    systemctl --user disable --now lilith-tts-daemon.service 2>/dev/null || true
    rm -f ~/.config/systemd/user/lilith-tts-daemon.service
    systemctl --user daemon-reload
    update-desktop-database ~/.local/share/applications 2>/dev/null || true
    gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor 2>/dev/null || true

# Add current user to input group for global hotkey
setup-hotkey:
    @if groups "$USER" | grep -qw 'input'; then \
        echo "User is already in the 'input' group."; \
    else \
        echo "Adding user to 'input' group (requires sudo)..."; \
        sudo usermod -aG input "$USER"; \
        echo "Added! Please log out and back in for this to take effect."; \
    fi

# Interactively check and download the NeuTTS Nano GGUF model (~120MB)
download-model:
    ./install.sh --only-model
