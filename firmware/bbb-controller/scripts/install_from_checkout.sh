#!/usr/bin/env bash
# Install slitcam-bbb-controller on a BeagleBone Black running Debian/Ubuntu.
#
# Run as root (or the script will re-exec itself under sudo):
#   bash firmware/bbb-controller/scripts/install_from_checkout.sh
set -euo pipefail

if [[ ${EUID:-$(id -u)} -ne 0 ]]; then
  exec sudo "$0" "$@"
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
SERVICE_NAME="slitcam-bbb-controller.service"
INSTALL_BIN="/usr/local/bin/slitcam-bbb-controller"
ENV_DIR="/etc/slitcam"
ENV_FILE="${ENV_DIR}/bbb-controller.env"
ENV_DEFAULT="${ENV_DIR}/bbb-controller.env.default"
PREBUILT_BIN_DEFAULT="${PROJECT_DIR}/prebuilt/linux-armv7/slitcam-bbb-controller"
FIRMWARE_SOURCE_BIN=""
BUILD_FROM_SOURCE=1
CARGO_BUILD_PROFILE="${SLITCAM_CARGO_BUILD_PROFILE:-bbb-release}"
CARGO_BUILD_JOBS="${SLITCAM_CARGO_BUILD_JOBS:-1}"

log() {
  printf '\n[%s] %s\n' "$(date '+%H:%M:%S')" "$1"
}

# ── firmware binary source ────────────────────────────────────────────────────

select_firmware_source() {
  if [[ -n "${SLITCAM_PREBUILT_BIN:-}" ]]; then
    if [[ ! -f "${SLITCAM_PREBUILT_BIN}" ]]; then
      echo "Configured prebuilt binary does not exist: ${SLITCAM_PREBUILT_BIN}" >&2
      exit 1
    fi
    FIRMWARE_SOURCE_BIN="${SLITCAM_PREBUILT_BIN}"
    BUILD_FROM_SOURCE=0
    return
  fi

  if [[ -f "${PREBUILT_BIN_DEFAULT}" ]]; then
    FIRMWARE_SOURCE_BIN="${PREBUILT_BIN_DEFAULT}"
    BUILD_FROM_SOURCE=0
    return
  fi

  FIRMWARE_SOURCE_BIN="${PROJECT_DIR}/target/${CARGO_BUILD_PROFILE}/slitcam-bbb-controller"
  BUILD_FROM_SOURCE=1
}

# ── OS packages ───────────────────────────────────────────────────────────────

ensure_packages() {
  log "Installing OS packages"
  apt-get update
  apt-get install -y \
    build-essential \
    ca-certificates \
    git \
    i2c-tools \
    v4l-utils

  if [[ "${BUILD_FROM_SOURCE}" -eq 1 ]]; then
    apt-get install -y cargo rustc
  fi
}

# ── BeagleBone cape/overlay checks ───────────────────────────────────────────

ensure_i2c_enabled() {
  log "Checking I2C bus availability"
  if [[ ! -e /dev/i2c-2 ]]; then
    echo "WARNING: /dev/i2c-2 not found." >&2
    echo "  Enable I2C2 via a BeagleBone device tree overlay or cape." >&2
    echo "  For Debian: add 'cape_universal=enable' to /boot/uEnv.txt." >&2
  else
    log "Found /dev/i2c-2"
  fi
}

ensure_uart_enabled() {
  log "Checking UART availability"
  if [[ ! -e /dev/ttyS1 ]]; then
    echo "WARNING: /dev/ttyS1 not found." >&2
    echo "  Enable UART1 via a BeagleBone device tree overlay or cape." >&2
  else
    log "Found /dev/ttyS1"
  fi
}

# ── build ─────────────────────────────────────────────────────────────────────

build_firmware() {
  if [[ "${BUILD_FROM_SOURCE}" -eq 0 ]]; then
    log "Using prebuilt firmware binary at ${FIRMWARE_SOURCE_BIN}"
    return
  fi

  log "Building Rust firmware with profile ${CARGO_BUILD_PROFILE} and ${CARGO_BUILD_JOBS} job(s)"
  CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS}" \
    cargo build --profile "${CARGO_BUILD_PROFILE}" --manifest-path "${PROJECT_DIR}/Cargo.toml"
}

# ── install assets ────────────────────────────────────────────────────────────

install_assets() {
  log "Installing binary and service assets"
  install -d /usr/local/bin
  install -m 0755 "${FIRMWARE_SOURCE_BIN}" "${INSTALL_BIN}"

  install -d "${ENV_DIR}"
  install -m 0644 "${PROJECT_DIR}/deploy/bbb-controller.env" "${ENV_DEFAULT}"
  if [[ ! -f "${ENV_FILE}" ]]; then
    install -m 0644 "${PROJECT_DIR}/deploy/bbb-controller.env" "${ENV_FILE}"
  fi

  install -m 0644 \
    "${PROJECT_DIR}/deploy/${SERVICE_NAME}" \
    "/etc/systemd/system/${SERVICE_NAME}"
}

enable_service() {
  log "Enabling ${SERVICE_NAME}"
  systemctl daemon-reload
  systemctl enable "${SERVICE_NAME}"
}

# ── entry point ───────────────────────────────────────────────────────────────

main() {
  select_firmware_source
  ensure_packages
  ensure_i2c_enabled
  ensure_uart_enabled
  build_firmware
  install_assets
  enable_service

  log "Install complete"
  echo "Reboot the BeagleBone, then connect the Pi Zero 2 W USB data port."
  echo "Check service status with: journalctl -u ${SERVICE_NAME} -f"
}

main "$@"
