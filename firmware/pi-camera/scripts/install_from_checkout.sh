#!/usr/bin/env bash
set -euo pipefail

if [[ ${EUID:-$(id -u)} -ne 0 ]]; then
  exec sudo "$0" "$@"
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
BOOT_CONFIG=""
BOOT_CMDLINE=""
SERVICE_NAME="slitcam-pi-camera.service"
INSTALL_BIN="/usr/local/bin/slitcam-pi-camera"
ENV_DIR="/etc/slitcam"
ENV_FILE="${ENV_DIR}/pi-camera.env"
ENV_DEFAULT="${ENV_DIR}/pi-camera.env.default"
UVC_REPO_DIR="/usr/local/src/uvc-gadget"

log() {
  printf '\n[%s] %s\n' "$(date '+%H:%M:%S')" "$1"
}

detect_boot_config() {
  if [[ -f /boot/firmware/config.txt ]]; then
    BOOT_CONFIG="/boot/firmware/config.txt"
  elif [[ -f /boot/config.txt ]]; then
    BOOT_CONFIG="/boot/config.txt"
  else
    echo "Unable to locate Raspberry Pi boot config.txt" >&2
    exit 1
  fi
}

detect_boot_cmdline() {
  if [[ -f /boot/firmware/cmdline.txt ]]; then
    BOOT_CMDLINE="/boot/firmware/cmdline.txt"
  elif [[ -f /boot/cmdline.txt ]]; then
    BOOT_CMDLINE="/boot/cmdline.txt"
  else
    echo "Unable to locate Raspberry Pi boot cmdline.txt" >&2
    exit 1
  fi
}

ensure_packages() {
  log "Installing OS packages"
  apt update
  apt install -y \
    build-essential \
    ca-certificates \
    cargo \
    git \
    libcamera-dev \
    libjpeg-dev \
    meson \
    ninja-build \
    pkg-config \
    rustc
}

ensure_otg_overlay() {
  detect_boot_config
  log "Ensuring USB peripheral gadget mode is enabled in ${BOOT_CONFIG}"

  if grep -Eq '^[[:space:]]*dtoverlay=dwc2,dr_mode=peripheral([[:space:]]*)$' "$BOOT_CONFIG"; then
    return
  fi

  # Append a final [all] block so the gadget overlay applies to Pi Zero 2 W
  # regardless of earlier model-specific sections in the stock config.
  printf '\n# SlitCam USB webcam gadget mode\n[all]\ndtoverlay=dwc2,dr_mode=peripheral\n' >> "$BOOT_CONFIG"
}

ensure_cmdline_loads_dwc2() {
  detect_boot_cmdline
  log "Ensuring dwc2 loads at boot via ${BOOT_CMDLINE}"

  local cmdline
  cmdline="$(tr -s ' ' < "${BOOT_CMDLINE}" | sed 's/^ //; s/ $//')"

  if grep -Eq '(^| )modules-load=([^ ]*,)?dwc2(,[^ ]*)?( |$)' "${BOOT_CMDLINE}"; then
    return
  fi

  if grep -Eq '(^| )modules-load=' "${BOOT_CMDLINE}"; then
    cmdline="$(printf '%s' "${cmdline}" | sed -E 's/(^| )modules-load=([^ ]*)/\1modules-load=\2,dwc2/')"
  else
    cmdline="${cmdline} modules-load=dwc2"
  fi

  printf '%s\n' "${cmdline}" > "${BOOT_CMDLINE}"
}

disable_conflicting_gadget_mode() {
  log "Disabling conflicting Ethernet gadget mode if present"

  if command -v rpi-usb-gadget >/dev/null 2>&1; then
    rpi-usb-gadget off || true
  fi

  if systemctl list-unit-files | grep -q '^rpi-usb-gadget\.service'; then
    systemctl disable --now rpi-usb-gadget.service || true
  fi
}

build_uvc_gadget() {
  log "Building uvc-gadget"
  mkdir -p /usr/local/src

  if [[ -d "${UVC_REPO_DIR}/.git" ]]; then
    git -C "${UVC_REPO_DIR}" pull --ff-only
  else
    rm -rf "${UVC_REPO_DIR}"
    git clone --depth=1 https://gitlab.freedesktop.org/camera/uvc-gadget.git "${UVC_REPO_DIR}"
  fi

  if [[ -d "${UVC_REPO_DIR}/build" ]]; then
    meson setup "${UVC_REPO_DIR}/build" "${UVC_REPO_DIR}" --prefix=/usr/local --buildtype=release --reconfigure
  else
    meson setup "${UVC_REPO_DIR}/build" "${UVC_REPO_DIR}" --prefix=/usr/local --buildtype=release
  fi

  meson compile -C "${UVC_REPO_DIR}/build"
  meson install -C "${UVC_REPO_DIR}/build"
  ldconfig
}

verify_camera() {
  log "Checking Raspberry Pi camera availability"
  if ! rpicam-hello --list-cameras; then
    echo "Camera check failed. Verify the Camera Module 3 is detected before enabling the service." >&2
    exit 1
  fi
}

build_firmware() {
  log "Building Rust firmware"
  cargo build --release --manifest-path "${PROJECT_DIR}/Cargo.toml"
}

install_assets() {
  log "Installing binary and service assets"
  install -d /usr/local/bin
  install -m 0755 "${PROJECT_DIR}/target/release/slitcam-pi-camera" "${INSTALL_BIN}"

  install -d "${ENV_DIR}"
  install -m 0644 "${PROJECT_DIR}/deploy/pi-camera.env" "${ENV_DEFAULT}"
  if [[ ! -f "${ENV_FILE}" ]]; then
    install -m 0644 "${PROJECT_DIR}/deploy/pi-camera.env" "${ENV_FILE}"
  fi

  install -m 0644 \
    "${PROJECT_DIR}/deploy/slitcam-pi-camera.service" \
    "/etc/systemd/system/${SERVICE_NAME}"
}

enable_service() {
  log "Enabling ${SERVICE_NAME}"
  systemctl daemon-reload
  systemctl enable "${SERVICE_NAME}"
}

main() {
  ensure_packages
  verify_camera
  ensure_otg_overlay
  ensure_cmdline_loads_dwc2
  disable_conflicting_gadget_mode
  build_uvc_gadget
  build_firmware
  install_assets
  enable_service

  log "Install complete"
  echo "Reboot the Pi, then connect the Pi Zero 2 W using the USB data port."
}

main "$@"
