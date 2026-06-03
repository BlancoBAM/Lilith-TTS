# Lilith-TTS 🌙

A high-performance, system-wide local Text-to-Speech application for Lilith Linux. 

![Lilith TTS Icon](./assets/icon.png)

## Features

- **NeuTTS Nano Integration**: Real-time high-quality TTS using the NeuTTS Nano model (~120MB GGUF).
- **Instant Voice Cloning**: Create new voices from 5–15 second clean WAV reference clips.
- **Global Hotkey**: Press `Ctrl+T+T+M` to read the current screen content or selection from any application.
- **Intelligent Screen Reader**: Uses AT-SPI2 to walk the accessibility tree, automatically skipping UI chrome, navigation bars, and advertisements.
- **Web-Optimized**: Heuristics to extract the main article content from browsers, ignoring sidebar clutter.
- **COSMIC Panel Applet**: Sleek and modern GUI integrated directly into the COSMIC Panel, featuring a clean white-flame symbolic icon.
- **Anchored Popup Window**: The main control panel and engine settings slide out beautifully, anchored directly to the panel applet.

## Architecture

Lilith-TTS is built as a three-crate Rust workspace:

- **`tts-core`**: Shared library containing the TTS engine abstraction, configuration, audio playback logic, and screen/clipboard readers.
- **`tts-daemon`**: Background service that monitors global hotkeys, manages the TTS engine lifecycle, and provides a Unix socket IPC server.
- **`tts-gui`**: A borderless panel applet that provides a user interface to control the daemon and configure voices.

## Installation

Lilith-TTS supports standard COSMIC applet installation using `just`, as well as a one-shot install script.

### Method 1: Using `just` (Standard COSMIC Applet Installation)

If you have `just` installed on your system, you can build and install the applet directly:

1. **Build the release binaries**:
   ```bash
   just
   ```

2. **Install the applet**:
   - **System-wide** (requires sudo, defaults to `prefix=/usr`):
     ```bash
     sudo just install
     # Or install to /usr/local
     sudo just install prefix=/usr/local
     ```
   - **User-local** (no root privileges required, installs to `~/.local`):
     ```bash
     just install-user
     ```

3. **Configure permissions for the global hotkey**:
   ```bash
   just setup-hotkey
   ```
   *(Note: You must log out and back in for the hotkey input group change to take effect)*

4. **Download the NeuTTS Nano model**:
   ```bash
   just download-model
   ```

### Method 2: Using the One-Shot Installer Script

Alternatively, you can run the interactive `install.sh` script, which automates building, local installation, group configuration, and model downloading in one step:

```bash
./install.sh
```

#### Installer Options:
- `--only-model`: Skips build and installation, jumping straight to checking/downloading the NeuTTS model.
- `--non-interactive`: Runs the installation in headless mode, downloading the model to your user-local directory without prompts.
- `-h, --help`: Displays help message.

---

## Setup & Configuration

### 1. Add the Applet to the COSMIC Panel
Once installed, Lilith TTS will be registered as a COSMIC applet:
1. Right-click any COSMIC panel and select **Add Applet** (or open **System Settings → Panels → Add Applet**).
2. Select **Lilith TTS** from the list.
3. The custom flame silhouette symbolic icon will appear in the panel. Click it to open the control popup.

### 2. NeuTTS Model Path
The high-quality NeuTTS Nano model (`neutts-nano-q4.gguf`) will search:
- System location: `/var/lib/lilith/models/neutts-nano-q4.gguf`
- User-local location: `~/.local/share/lilith-tts/models/neutts-nano-q4.gguf`

You can download it using the **Download NeuTTS Nano** button inside the applet's Settings page, or manually point to a pre-downloaded model path from there.

### 3. Daemon Control
The background daemon handles the global hotkey (`Ctrl + T + T + M`) and reads text from the active window/clipboard. It runs as a systemd user service:
```bash
# Check status
systemctl --user status lilith-tts-daemon

# Restart daemon
systemctl --user restart lilith-tts-daemon
```

---
*Built for Lilith Linux — BlancoBAM*

