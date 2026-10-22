#!/bin/sh
# ==============================================================================
# Neth Server Installer for FreeBSD / BSD Systems
# Developed and maintained by KIYOVN | https://kiyomc.com
# ==============================================================================

set -eu

INSTALL_DIR="/usr/local/neth"
CONFIG_FILE="${INSTALL_DIR}/neth.toml"
RC_SCRIPT="/usr/local/etc/rc.d/neth"
USER_NAME="neth"
GROUP_NAME="neth"

echo "=========================================================="
echo "          Neth Server Installer - FreeBSD Edition         "
echo "           High-Performance Minecraft Engine              "
echo "             KIYOVN | https://kiyomc.com                   "
echo "=========================================================="
echo ""

if [ "$(id -u)" -ne 0 ]; then
    echo "Error: This installer must be run as root." >&2
    exit 1
fi

echo "[1/5] Setting up system account '${USER_NAME}'..."
if ! pw group show "${GROUP_NAME}" >/dev/null 2>&1; then
    pw groupadd "${GROUP_NAME}"
fi

if ! pw user show "${USER_NAME}" >/dev/null 2>&1; then
    pw useradd "${USER_NAME}" -g "${GROUP_NAME}" -d "${INSTALL_DIR}" -s /usr/sbin/nologin -c "Neth Minecraft Server"
fi

echo "[2/5] Creating directories..."
mkdir -p "${INSTALL_DIR}/world" "${INSTALL_DIR}/logs" "${INSTALL_DIR}/plugins"

echo "[3/5] Setting up binary in /usr/local/bin/neth..."
LOCAL_RELEASE="$(dirname "$0")/../target/release/neth"
if [ -f "${LOCAL_RELEASE}" ]; then
    cp "${LOCAL_RELEASE}" /usr/local/bin/neth
else
    echo "Notice: Compiling from source or downloading..."
    if command -v cargo >/dev/null 2>&1; then
        (cd "$(dirname "$0")/.." && cargo build --release --bin neth)
        cp "${LOCAL_RELEASE}" /usr/local/bin/neth
    else
        echo "Error: Please install cargo or provide pre-built neth binary." >&2
        exit 1
    fi
fi
chmod 755 /usr/local/bin/neth

echo "[4/5] Initializing configuration..."
if [ ! -f "${CONFIG_FILE}" ]; then
    SOURCE_CONFIG="$(dirname "$0")/../neth.toml"
    if [ -f "${SOURCE_CONFIG}" ]; then
        cp "${SOURCE_CONFIG}" "${CONFIG_FILE}"
    else
        cat <<'EOF' > "${CONFIG_FILE}"
# Neth Server Configuration (FreeBSD)
# Website: https://kiyomc.com
server_name = "Neth Server on FreeBSD"
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

echo "[5/5] Creating rc.d service script (${RC_SCRIPT})..."
cat <<'EOF' > "${RC_SCRIPT}"
#!/bin/sh
#
# PROVIDE: neth
# REQUIRE: LOGIN DAEMON NETWORKING
# KEYWORD: shutdown

. /etc/rc.subr

name="neth"
rcvar="neth_enable"

load_rc_config $name

: ${neth_enable:="NO"}
: ${neth_user:="neth"}
: ${neth_chdir:="/usr/local/neth"}

command="/usr/local/bin/neth"
command_args="> /dev/null 2>&1 &"

run_rc_command "$1"
EOF
chmod 755 "${RC_SCRIPT}"

echo ""
echo "=========================================================="
echo "            Neth Server Installed Successfully!           "
echo "=========================================================="
echo "Binary Location:        /usr/local/bin/neth"
echo "Working Directory:      ${INSTALL_DIR}"
echo "Configuration:          ${CONFIG_FILE}"
echo "Enable Service:         sysrc neth_enable=YES"
echo "Start Service:          service neth start"
echo "Official Website:       https://kiyomc.com"
echo "=========================================================="
echo ""
