#!/usr/bin/env bash
# UVC gadget setup and launch script for Raspberry Pi Zero 2 W.
#
# Sets up the USB configfs gadget (same IDs and frame descriptors as the Rust
# slitcam-pi-camera service) then runs uvc-gadget in the foreground so that
# systemd can track the process and restart it on failure.
#
# Install:
#   sudo cp rpi-uvc-gadget.sh /usr/local/bin/rpi-uvc-gadget.sh
#   sudo chmod +x /usr/local/bin/rpi-uvc-gadget.sh
set -euo pipefail

# ── configuration ─────────────────────────────────────────────────────────────

GADGET_NAME="${SLITCAM_USB_GADGET_NAME:-slitcam0}"
USB_VENDOR_ID="${SLITCAM_USB_VENDOR_ID:-0x0525}"
USB_PRODUCT_ID="${SLITCAM_USB_PRODUCT_ID:-0xa4a2}"
USB_SERIAL="${SLITCAM_USB_SERIAL:-SLITCAM-0001}"
USB_MANUFACTURER="${SLITCAM_USB_MANUFACTURER:-$(cat /proc/sys/kernel/hostname 2>/dev/null || echo slitcam-pi)}"
USB_PRODUCT="${SLITCAM_USB_PRODUCT:-SlitCam Pi Camera}"
USB_CONFIGURATION="${SLITCAM_USB_CONFIGURATION:-UVC}"
MAX_POWER_MA="${SLITCAM_USB_MAX_POWER_MA:-500}"
CAMERA_ID="${SLITCAM_CAMERA_ID:-0}"
UVC_GADGET_BIN="${SLITCAM_UVC_GADGET_BIN:-/usr/local/bin/uvc-gadget}"
UDC_WAIT_SECS="${SLITCAM_UDC_WAIT_SECS:-60}"

CONFIGFS_ROOT="/sys/kernel/config"
GADGET_DIR="${CONFIGFS_ROOT}/usb_gadget/${GADGET_NAME}"

log()  { printf '[%s] %s\n' "$(date '+%H:%M:%S')" "$1" >&2; }
die()  { log "ERROR: $1"; exit 1; }
warn() { log "WARN:  $1"; }

# ── cleanup ───────────────────────────────────────────────────────────────────

cleanup() {
    log "Cleaning up gadget ${GADGET_NAME}"
    # Unbind the UDC before removing the gadget tree.
    local udc_file="${GADGET_DIR}/UDC"
    if [[ -f "${udc_file}" ]]; then
        echo "" > "${udc_file}" 2>/dev/null || true
    fi
    if [[ -d "${GADGET_DIR}" ]]; then
        rm -rf "${GADGET_DIR}" 2>/dev/null || true
    fi
}

# Register cleanup for normal exit and signals so systemd ExecStop is not
# required to leave configfs in a consistent state.
trap cleanup EXIT

# ── preflight ─────────────────────────────────────────────────────────────────

[[ ${EUID:-$(id -u)} -eq 0 ]] || die "must be run as root"

[[ -d "${CONFIGFS_ROOT}" ]] \
    || die "configfs not mounted at ${CONFIGFS_ROOT}; is sys-kernel-config.mount active?"

[[ -x "${UVC_GADGET_BIN}" ]] \
    || die "uvc-gadget binary not found at ${UVC_GADGET_BIN}; run the install script first"

# ── wait for UDC ─────────────────────────────────────────────────────────────

wait_for_udc() {
    log "Waiting up to ${UDC_WAIT_SECS}s for a USB device controller..."
    local deadline=$(( $(date +%s) + UDC_WAIT_SECS ))
    while true; do
        local udc
        udc="$(ls /sys/class/udc 2>/dev/null | head -n1 || true)"
        if [[ -n "${udc}" ]]; then
            log "Found UDC: ${udc}"
            echo "${udc}"
            return 0
        fi
        if [[ $(date +%s) -ge ${deadline} ]]; then
            die "no USB device controller found under /sys/class/udc after ${UDC_WAIT_SECS} seconds; connect the USB host to the Pi Zero 2 W data port"
        fi
        sleep 1
    done
}

# ── module loading ────────────────────────────────────────────────────────────

log "Loading kernel modules"
modprobe dwc2   2>/dev/null || warn "dwc2 already loaded or unavailable"
modprobe libcomposite

# ── gadget teardown (idempotent) ──────────────────────────────────────────────

if [[ -d "${GADGET_DIR}" ]]; then
    warn "Stale gadget directory found; removing before setup"
    local_udc_file="${GADGET_DIR}/UDC"
    [[ -f "${local_udc_file}" ]] && echo "" > "${local_udc_file}" 2>/dev/null || true
    rm -rf "${GADGET_DIR}"
fi

# ── UDC discovery (after modules are loaded) ─────────────────────────────────

UDC_NAME="$(wait_for_udc)"

# ── gadget root ───────────────────────────────────────────────────────────────

log "Creating gadget ${GADGET_NAME}"
mkdir -p "${GADGET_DIR}"

echo "${USB_VENDOR_ID}"  > "${GADGET_DIR}/idVendor"
echo "${USB_PRODUCT_ID}" > "${GADGET_DIR}/idProduct"
echo "0x0100"            > "${GADGET_DIR}/bcdDevice"
echo "0x0200"            > "${GADGET_DIR}/bcdUSB"

mkdir -p "${GADGET_DIR}/strings/0x409"
echo "${USB_SERIAL}"        > "${GADGET_DIR}/strings/0x409/serialnumber"
echo "${USB_MANUFACTURER}"  > "${GADGET_DIR}/strings/0x409/manufacturer"
echo "${USB_PRODUCT}"       > "${GADGET_DIR}/strings/0x409/product"

# ── configuration ─────────────────────────────────────────────────────────────

mkdir -p "${GADGET_DIR}/configs/c.1/strings/0x409"
echo "${USB_CONFIGURATION}" > "${GADGET_DIR}/configs/c.1/strings/0x409/configuration"
echo "${MAX_POWER_MA}"      > "${GADGET_DIR}/configs/c.1/MaxPower"

# ── UVC function ──────────────────────────────────────────────────────────────

F="${GADGET_DIR}/functions/uvc.0"

mkdir -p "${F}/control/header/h"
mkdir -p "${F}/control/class/fs"
mkdir -p "${F}/control/class/ss"
mkdir -p "${F}/streaming/header/h"
mkdir -p "${F}/streaming/class/fs"
mkdir -p "${F}/streaming/class/hs"
mkdir -p "${F}/streaming/class/ss"

# Helper: create one uncompressed or MJPEG frame descriptor.
# Usage: add_frame <format_dir> <frame_name_prefix> <width> <height> <intervals…>
add_frame() {
    local fmt="$1" name="$2" w="$3" h="$4"
    shift 4
    local fdir="${F}/streaming/${fmt}/${name}/${h}p"
    mkdir -p "${fdir}"
    echo "${w}"                      > "${fdir}/wWidth"
    echo "${h}"                      > "${fdir}/wHeight"
    echo $(( w * h * 2 ))           > "${fdir}/dwMaxVideoFrameBufferSize"
    printf '%s\n' "$@"              > "${fdir}/dwFrameInterval"
}

# Uncompressed (YUYV) modes — must match FRAME_SPECS in gadget.rs
add_frame uncompressed u  640  480  333333 416667 500000 666666 1000000 1333333 2000000
add_frame uncompressed u 1280  720  1000000 1333333 2000000
add_frame uncompressed u 1920 1080  2000000

# MJPEG modes
add_frame mjpeg m  640  480  333333 416667 500000 666666 1000000 1333333 2000000
add_frame mjpeg m 1280  720  333333 416667 500000 666666 1000000 1333333 2000000
add_frame mjpeg m 1920 1080  333333 416667 500000 666666 1000000 1333333 2000000

# Link format directories into the streaming header
ln -s "${F}/streaming/uncompressed/u" "${F}/streaming/header/h/u"
ln -s "${F}/streaming/mjpeg/m"        "${F}/streaming/header/h/m"

# Link streaming header into each speed class
ln -s "${F}/streaming/header/h" "${F}/streaming/class/fs/h"
ln -s "${F}/streaming/header/h" "${F}/streaming/class/hs/h"
ln -s "${F}/streaming/header/h" "${F}/streaming/class/ss/h"

# Link control header into each speed class
ln -s "${F}/control/header/h" "${F}/control/class/fs/h"
ln -s "${F}/control/header/h" "${F}/control/class/ss/h"

echo "2048" > "${F}/streaming_maxpacket"

# Link the function into the configuration
ln -s "${F}" "${GADGET_DIR}/configs/c.1/uvc.0"

# ── bind to UDC ───────────────────────────────────────────────────────────────

log "Binding gadget to UDC ${UDC_NAME}"
echo "${UDC_NAME}" > "${GADGET_DIR}/UDC"

log "Gadget bound; starting uvc-gadget (camera ${CAMERA_ID})"

# Run uvc-gadget in the foreground.  systemd tracks this process; if it exits
# the service restarts per Restart=on-failure.
exec "${UVC_GADGET_BIN}" -c "${CAMERA_ID}" uvc.0
