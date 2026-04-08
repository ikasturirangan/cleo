#!/usr/bin/env bash
# SlitCam Pi installer — run once on a fresh Raspberry Pi OS Lite (64-bit).
# Usage: bash <(curl -fsSL https://raw.githubusercontent.com/ikasturirangan/cleo/main/firmware/pi-camera/scripts/install.sh)
set -euo pipefail

REPO=https://raw.githubusercontent.com/ikasturirangan/cleo/main

log()  { echo "[$(date '+%H:%M:%S')] $1"; }
die()  { echo "[ERROR] $1"; exit 1; }

# ── 1. Boot config ────────────────────────────────────────────────────────────

log "Configuring boot: USB OTG + UART"
CONFIG=/boot/firmware/config.txt

sudo sed -i '/dtoverlay=dwc2/d' "${CONFIG}"
echo "dtoverlay=dwc2,dr_mode=peripheral" | sudo tee -a "${CONFIG}"

sudo raspi-config nonint do_serial_hw 0
sudo raspi-config nonint do_serial_cons 1

# ── 2. Dependencies ───────────────────────────────────────────────────────────

log "Installing dependencies"
sudo apt-get update -qq
sudo apt-get install -y \
    python3-picamera2 \
    iproute2

# ── 3. Camera stream script ───────────────────────────────────────────────────

log "Installing camera stream script"
curl -fsSL "${REPO}/firmware/pi-camera/scripts/camera-stream.py" \
    | sudo tee /usr/local/bin/slitcam-camera-stream.py > /dev/null
sudo chmod +x /usr/local/bin/slitcam-camera-stream.py
log "camera-stream.py installed"

# ── 4. slitcam-pi-camera binary (pre-built) ───────────────────────────────────

log "Downloading slitcam-pi-camera"
curl -fsSL "${REPO}/firmware/pi-camera/deploy/bin/slitcam-pi-camera" \
    -o /tmp/slitcam-pi-camera
sudo install -m 755 /tmp/slitcam-pi-camera /usr/local/bin/slitcam-pi-camera
log "slitcam-pi-camera installed"

# ── 5. Env config ─────────────────────────────────────────────────────────────

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
SLITCAM_STREAM_PORT=8080
SLITCAM_STREAM_WIDTH=640
SLITCAM_STREAM_HEIGHT=480
SLITCAM_STREAM_FPS=30
SLITCAM_UART_DEVICE=/dev/serial0
SLITCAM_SG_THRESHOLD=255
EOF

# ── 6. Systemd services ───────────────────────────────────────────────────────

log "Installing systemd services"
curl -fsSL "${REPO}/firmware/pi-camera/scripts/rpi-uvc-gadget.sh" \
    | sudo tee /usr/local/bin/rpi-uvc-gadget.sh > /dev/null
sudo chmod +x /usr/local/bin/rpi-uvc-gadget.sh

curl -fsSL "${REPO}/firmware/pi-camera/deploy/uvc-gadget.service" \
    | sudo tee /etc/systemd/system/uvc-gadget.service > /dev/null
curl -fsSL "${REPO}/firmware/pi-camera/deploy/slitcam-motor.service" \
    | sudo tee /etc/systemd/system/slitcam-motor.service > /dev/null

sudo systemctl daemon-reload
sudo systemctl enable uvc-gadget.service slitcam-motor.service

log "Services enabled"

# ── 7. Done ───────────────────────────────────────────────────────────────────

log "Install complete. Rebooting in 5 seconds..."
sleep 5
sudo reboot
