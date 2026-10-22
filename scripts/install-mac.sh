#!/usr/bin/env bash
# ==============================================================================
# Neth Server Installer for macOS (Apple Silicon & Intel)
# Developed and maintained by KIYOVN | https://kiyomc.com
# ==============================================================================

set -euo pipefail

INSTALL_DIR="${HOME}/Library/Application Support/neth"
CONFIG_FILE="${INSTALL_DIR}/neth.toml"
PLIST_FILE="${HOME}/Library/LaunchAgents/com.kiyomc.neth.plist"
BIN_DIR="/usr/local/bin"

echo "=========================================================="
echo "          Neth Server Installer - macOS Edition           "
echo "           High-Performance Minecraft Engine              "
echo "             KIYOVN | https://kiyomc.com                   "
echo "=========================================================="
echo ""

# 1. Architecture detection
ARCH="$(uname -m)"
case "${ARCH}" in
    arm64)
        TARGET_ARCH="aarch64"
        echo "[1/5] Detected Apple Silicon (M-series / ARM64)"
        if [ -d "/opt/homebrew/bin" ]; then
            BIN_DIR="/opt/homebrew/bin"
        fi
        ;;
    x86_64)
        TARGET_ARCH="x86_64"
        echo "[1/5] Detected Intel Mac (x86_64)"
        ;;
    *)
        echo "Error: Unsupported architecture: ${ARCH}" >&2
        exit 1
        ;;
esac

# 2. Directory structure
echo "[2/5] Creating directory tree in ${INSTALL_DIR}..."
mkdir -p "${INSTALL_DIR}/world" "${INSTALL_DIR}/logs" "${INSTALL_DIR}/plugins"
mkdir -p "${HOME}/Library/LaunchAgents"

# 3. Binary placement
echo "[3/5] Setting up Neth executable in ${BIN_DIR}/neth..."
mkdir -p "${BIN_DIR}"

LOCAL_RELEASE="$(dirname "$0")/../target/release/neth"
LOCAL_DEBUG="$(dirname "$0")/../target/debug/neth"

if [ -f "${LOCAL_RELEASE}" ]; then
    echo "      Using locally compiled release binary: ${LOCAL_RELEASE}"
    cp "${LOCAL_RELEASE}" "${BIN_DIR}/neth"
elif [ -f "${LOCAL_DEBUG}" ]; then
    echo "      Using locally compiled debug binary: ${LOCAL_DEBUG}"
    cp "${LOCAL_DEBUG}" "${BIN_DIR}/neth"
else
    DOWNLOAD_URL="https://github.com/kiyogo/neth/releases/latest/download/neth-macos-${TARGET_ARCH}"
    echo "      Downloading macOS binary from: ${DOWNLOAD_URL}"
    curl -fsSL "${DOWNLOAD_URL}" -o "${BIN_DIR}/neth"
fi

chmod +x "${BIN_DIR}/neth"

# 4. Default configuration
echo "[4/5] Setting up configuration in ${CONFIG_FILE}..."
if [ ! -f "${CONFIG_FILE}" ]; then
    SOURCE_CONFIG="$(dirname "$0")/../neth.toml"
    if [ -f "${SOURCE_CONFIG}" ]; then
        cp "${SOURCE_CONFIG}" "${CONFIG_FILE}"
    else
        cat <<'EOF' > "${CONFIG_FILE}"
# Neth Server Configuration
# Website: https://kiyomc.com
server_name = "Neth Server on macOS"
motd = "A Minecraft Server Powered by Neth (Rust)"

[network]
address = "0.0.0.0:25565"
online_mode = true
compression_threshold = 256

[bedrock]
enabled = true
address = "0.0.0.0:19132"
EOF
    fi
fi

# 5. Launch script and LaunchAgent daemon
echo "[5/5] Creating macOS launch helper and launchd agent..."
cat <<EOF > "${INSTALL_DIR}/run.command"
#!/bin/bash
cd "\$(dirname "\$0")"
"${BIN_DIR}/neth"
EOF
chmod +x "${INSTALL_DIR}/run.command"

cat <<EOF > "${PLIST_FILE}"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.kiyomc.neth</string>
    <key>ProgramArguments</key>
    <array>
        <string>${BIN_DIR}/neth</string>
    </array>
    <key>WorkingDirectory</key>
    <string>${INSTALL_DIR}</string>
    <key>StandardOutPath</key>
    <string>${INSTALL_DIR}/logs/launchd-stdout.log</string>
    <key>StandardErrorPath</key>
    <string>${INSTALL_DIR}/logs/launchd-stderr.log</string>
    <key>KeepAlive</key>
    <false/>
    <key>RunAtLoad</key>
    <false/>
</dict>
</plist>
EOF

echo ""
echo "=========================================================="
echo "            Neth Server Installed Successfully!           "
echo "=========================================================="
echo "Binary Location:        ${BIN_DIR}/neth"
echo "Working Directory:      ${INSTALL_DIR}"
echo "Configuration:          ${CONFIG_FILE}"
echo "Finder Launcher:        ${INSTALL_DIR}/run.command"
echo "Launchd Service:        launchctl load ${PLIST_FILE}"
echo "Official Website:       https://kiyomc.com"
echo "=========================================================="
echo ""
