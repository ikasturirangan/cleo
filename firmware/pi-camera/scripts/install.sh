#!/usr/bin/env bash
# SlitCam Pi installer — run once on a fresh Raspberry Pi OS Lite (64-bit).
# Usage: bash <(curl -fsSL https://raw.githubusercontent.com/ikasturirangan/cleo/main/firmware/pi-camera/scripts/install.sh)
set -euo pipefail

log()  { echo "[$(date '+%H:%M:%S')] $1"; }
die()  { echo "[ERROR] $1"; exit 1; }

# ── 1. Boot config ────────────────────────────────────────────────────────────

log "Configuring boot: USB OTG + UART"
CONFIG=/boot/firmware/config.txt

# Remove any existing dwc2 lines to avoid conflicts
sudo sed -i '/dtoverlay=dwc2/d' "${CONFIG}"
echo "dtoverlay=dwc2,dr_mode=peripheral" | sudo tee -a "${CONFIG}"

# Enable UART hardware, disable serial console
sudo raspi-config nonint do_serial_hw 0
sudo raspi-config nonint do_serial_cons 1

# ── 2. Dependencies ───────────────────────────────────────────────────────────

log "Installing dependencies"
sudo apt-get update -qq
sudo apt-get install -y \
    git cmake meson ninja-build \
    libcamera-dev libudev-dev \
    pkg-config build-essential \
    libjpeg-dev

# ── 3. uvc-gadget ─────────────────────────────────────────────────────────────

log "Building uvc-gadget"
rm -rf /tmp/uvc-gadget
git clone https://github.com/kbingham/uvc-gadget /tmp/uvc-gadget
cd /tmp/uvc-gadget
meson setup build -Dwerror=false
ninja -C build -j$(nproc) 2>&1 | tail -5
sudo cp build/src/uvc-gadget /usr/local/bin/
log "uvc-gadget installed at /usr/local/bin/uvc-gadget"

# ── 4. Rust ───────────────────────────────────────────────────────────────────

log "Installing Rust"
if ! command -v rustup &>/dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi
source "$HOME/.cargo/env"
rustup default stable

# ── 5. slitcam-pi-camera binary ───────────────────────────────────────────────

log "Building slitcam-pi-camera"
REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# If running from a fresh clone, REPO_DIR points to the repo.
# If piped via curl, clone the repo first.
if [[ ! -f "${REPO_DIR}/Cargo.toml" ]]; then
    log "Cloning slitcam repo"
    rm -rf /tmp/slitcam
    git clone https://github.com/ikasturirangan/cleo /tmp/slitcam
    REPO_DIR=/tmp/slitcam/firmware/pi-camera
fi

cd "${REPO_DIR}"
cargo build --release 2>&1 | tail -5
sudo cp target/release/slitcam-pi-camera /usr/local/bin/
log "slitcam-pi-camera installed"

# ── 6. Env config ─────────────────────────────────────────────────────────────

log "Writing /etc/slitcam/pi-camera.env"
sudo mkdir -p /etc/slitcam
sudo tee /etc/slitcam/pi-camera.env > /dev/null <<'EOF'
SLITCAM_GPIO_STEP=4
SLITCAM_GPIO_DIR=17
SLITCAM_GPIO_EN=27
SLITCAM_GPIO_DIAG=22
SLITCAM_STEP_DELAY_US=500
SLITCAM_HOME_MAX_STEPS=10000
SLITCAM_HOME_BACKOFF_STEPS=100
SLITCAM_UVC_RESOLUTION=640x480
SLITCAM_UVC_FRAMERATE=30
SLITCAM_UART_DEVICE=/dev/serial0
SLITCAM_SG_THRESHOLD=255
EOF

# ── 7. Systemd services ───────────────────────────────────────────────────────

log "Installing systemd services"
SCRIPTS_DIR="$(dirname "${BASH_SOURCE[0]}")"

sudo cp "${SCRIPTS_DIR}/rpi-uvc-gadget.sh" /usr/local/bin/rpi-uvc-gadget.sh
sudo chmod +x /usr/local/bin/rpi-uvc-gadget.sh

sudo cp "${SCRIPTS_DIR}/../deploy/uvc-gadget.service" /etc/systemd/system/
sudo cp "${SCRIPTS_DIR}/../deploy/slitcam-motor.service" /etc/systemd/system/

sudo systemctl daemon-reload
sudo systemctl enable uvc-gadget.service slitcam-motor.service

log "Services enabled"

# ── 8. Done ───────────────────────────────────────────────────────────────────

log "Install complete. Rebooting in 5 seconds..."
sleep 5
sudo reboot
