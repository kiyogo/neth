#!/usr/bin/env bash
# ==============================================================================
# Neth Server Installer for Linux (Ubuntu, Debian, RHEL, Fedora, Arch, Alpine)
# Developed and maintained by KIYOVN | https://kiyomc.com
# ==============================================================================

set -euo pipefail

INSTALL_DIR="/opt/neth"
CONFIG_FILE="${INSTALL_DIR}/neth.toml"
SERVICE_FILE="/etc/systemd/system/neth.service"
USER_NAME="neth"
GROUP_NAME="neth"

echo "=========================================================="
echo "          Neth Server Installer - Linux Edition           "
echo "           High-Performance Minecraft Engine              "
echo "             KIYOVN | https://kiyomc.com                   "
echo "=========================================================="
echo ""

# 1. Root privilege verification
if [ "$(id -u)" -ne 0 ]; then
    echo "Error: This installer must be run as root (or via sudo)." >&2
    exit 1
fi

# 2. Architecture detection
ARCH="$(uname -m)"
case "${ARCH}" in
    x86_64|amd64)
        TARGET_ARCH="x86_64"
        ;;
    aarch64|arm64)
        TARGET_ARCH="aarch64"
        ;;
    *)
        echo "Error: Unsupported architecture: ${ARCH}. Neth supports x86_64 and aarch64." >&2
        exit 1
        ;;
esac

echo "[1/6] Detected System: Linux (${TARGET_ARCH})"

# 3. Create dedicated system user
echo "[2/6] Configuring service account '${USER_NAME}'..."
if ! getent group "${GROUP_NAME}" >/dev/null 2>&1; then
    groupadd --system "${GROUP_NAME}"
fi

if ! getent passwd "${USER_NAME}" >/dev/null 2>&1; then
    useradd --system \
            --gid "${GROUP_NAME}" \
            --home-dir "${INSTALL_DIR}" \
            --no-create-home \
            --shell /usr/sbin/nologin \
            --comment "Neth Minecraft Server Service" \
            "${USER_NAME}"
fi

# 4. Prepare directory tree
echo "[3/6] Setting up directories in ${INSTALL_DIR}..."
mkdir -p "${INSTALL_DIR}/bin" "${INSTALL_DIR}/world" "${INSTALL_DIR}/logs" "${INSTALL_DIR}/plugins"

# 5. Binary placement
echo "[4/6] Installing Neth executable..."
LOCAL_RELEASE="$(dirname "$0")/../target/release/neth"
LOCAL_DEBUG="$(dirname "$0")/../target/debug/neth"

if [ -f "${LOCAL_RELEASE}" ]; then
    echo "      Using locally compiled release binary: ${LOCAL_RELEASE}"
    cp "${LOCAL_RELEASE}" /usr/local/bin/neth
elif [ -f "${LOCAL_DEBUG}" ]; then
    echo "      Using locally compiled debug binary: ${LOCAL_DEBUG}"
    cp "${LOCAL_DEBUG}" /usr/local/bin/neth
else
    DOWNLOAD_URL="https://github.com/kiyogo/neth/releases/latest/download/neth-linux-${TARGET_ARCH}"
    echo "      Fetching binary from: ${DOWNLOAD_URL}"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "${DOWNLOAD_URL}" -o /usr/local/bin/neth
    elif command -v wget >/dev/null 2>&1; then
        wget -qO /usr/local/bin/neth "${DOWNLOAD_URL}"
    else
        echo "Error: Neither curl nor wget is available." >&2
        exit 1
    fi
fi

chmod 755 /usr/local/bin/neth

# 6. Default configuration
echo "[5/6] Initializing configuration in ${CONFIG_FILE}..."
if [ ! -f "${CONFIG_FILE}" ]; then
    SOURCE_CONFIG="$(dirname "$0")/../neth.toml"
    if [ -f "${SOURCE_CONFIG}" ]; then
        cp "${SOURCE_CONFIG}" "${CONFIG_FILE}"
    else
        cat <<'EOF' > "${CONFIG_FILE}"
# Neth Server Configuration
# Website: https://kiyomc.com
server_name = "Neth Production Server"
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

chown -R "${USER_NAME}:${GROUP_NAME}" "${INSTALL_DIR}"
chmod 750 "${INSTALL_DIR}"

# 7. Systemd Service Setup
if command -v systemctl >/dev/null 2>&1; then
    echo "[6/6] Registering Systemd service (${SERVICE_FILE})..."
    cat <<EOF > "${SERVICE_FILE}"
[Unit]
Description=Neth Minecraft Server Engine (Rust)
Documentation=https://kiyomc.com
After=network.target network-online.target
Wants=network-online.target

[Service]
Type=simple
User=${USER_NAME}
Group=${GROUP_NAME}
WorkingDirectory=${INSTALL_DIR}
ExecStart=/usr/local/bin/neth
Restart=on-failure
RestartSec=5s

# Security Hardening
NoNewPrivileges=true
ProtectSystem=full
ProtectHome=true
PrivateTmp=true

# Resource Limits
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    systemctl enable neth.service
    echo "      Systemd service enabled."

    # Firewall configuration
    if command -v ufw >/dev/null 2>&1 && ufw status | grep -qw "active"; then
        ufw allow 25565/tcp comment "Neth Java Edition" >/dev/null 2>&1 || true
        ufw allow 19132/udp comment "Neth Bedrock Edition" >/dev/null 2>&1 || true
        echo "      UFW firewall rules added."
    elif command -v firewall-cmd >/dev/null 2>&1 && systemctl is-active --quiet firewalld; then
        firewall-cmd --permanent --add-port=25565/tcp >/dev/null 2>&1 || true
        firewall-cmd --permanent --add-port=19132/udp >/dev/null 2>&1 || true
        firewall-cmd --reload >/dev/null 2>&1 || true
        echo "      Firewalld rules added."
    fi
fi

echo ""
echo "=========================================================="
echo "            Neth Server Installed Successfully!           "
echo "=========================================================="
echo "Binary Location:        /usr/local/bin/neth"
echo "Server Home:            ${INSTALL_DIR}"
echo "Configuration:          ${CONFIG_FILE}"
echo "Start Service:          sudo systemctl start neth"
echo "View Live Logs:         sudo journalctl -u neth -f"
echo "Official Website:       https://kiyomc.com"
echo "=========================================================="
echo ""
