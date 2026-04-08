#!/usr/bin/env bash
# CDC NCM USB gadget + MJPEG camera stream for Raspberry Pi Zero 2 W.
#
# Exposes one USB function:
#   ncm.usb0 — USB ethernet (CDC NCM) at 192.168.7.1
#
# Camera is streamed as MJPEG over HTTP at http://192.168.7.1:8080/stream.mjpg
# using picamera2 (libcamera-based, no UVC/V4L2 compat needed).
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
USB_CONFIGURATION="${SLITCAM_USB_CONFIGURATION:-NCM}"
MAX_POWER_MA="${SLITCAM_USB_MAX_POWER_MA:-500}"
CAMERA_STREAM_BIN="${SLITCAM_CAMERA_STREAM_BIN:-/usr/local/bin/slitcam-camera-stream.py}"
UDC_WAIT_SECS="${SLITCAM_UDC_WAIT_SECS:-60}"

# CDC NCM — fixed MACs so the Mac always sees the same interface.
NCM_DEV_ADDR="${SLITCAM_NCM_DEV_ADDR:-42:61:61:61:61:01}"   # Pi side
NCM_HOST_ADDR="${SLITCAM_NCM_HOST_ADDR:-42:61:61:61:61:02}"  # Mac side
NCM_PI_IP="${SLITCAM_NCM_PI_IP:-192.168.7.1}"
NCM_PREFIX="${SLITCAM_NCM_PREFIX:-24}"

CONFIGFS_ROOT="/sys/kernel/config"
GADGET_DIR="${CONFIGFS_ROOT}/usb_gadget/${GADGET_NAME}"

log()  { printf '[%s] %s\n' "$(date '+%H:%M:%S')" "$1" >&2; }
die()  { log "ERROR: $1"; exit 1; }
warn() { log "WARN:  $1"; }

# ── cleanup ───────────────────────────────────────────────────────────────────

teardown_gadget() {
    local gdir="$1"
    [[ -d "${gdir}" ]] || return 0

    # 1. Bring down USB network interface before unbinding.
    ip link set usb0 down   2>/dev/null || true
    ip addr flush dev usb0  2>/dev/null || true

    # 2. Unbind from UDC.
    if [[ -f "${gdir}/UDC" ]]; then
        echo "" > "${gdir}/UDC" 2>/dev/null || true
        sleep 0.1
    fi

    # 3. Remove config symlink.
    rm -f "${gdir}/configs/c.1/ncm.usb0" 2>/dev/null || true

    # 4. Remove NCM function dir.
    rmdir "${gdir}/functions/ncm.usb0" 2>/dev/null || true

    # 5. Remove config strings then config.
    rmdir "${gdir}/configs/c.1/strings/0x409" 2>/dev/null || true
    rmdir "${gdir}/configs/c.1/strings"       2>/dev/null || true
    rmdir "${gdir}/configs/c.1"               2>/dev/null || true
    rmdir "${gdir}/configs"                   2>/dev/null || true

    # 6. Remove gadget strings then gadget root.
    rmdir "${gdir}/strings/0x409" 2>/dev/null || true
    rmdir "${gdir}/strings"       2>/dev/null || true
    rmdir "${gdir}"               2>/dev/null || true
}

cleanup() {
    log "Cleaning up gadget ${GADGET_NAME}"
    teardown_gadget "${GADGET_DIR}"
}

trap cleanup EXIT

# ── preflight ─────────────────────────────────────────────────────────────────

[[ ${EUID:-$(id -u)} -eq 0 ]] || die "must be run as root"

[[ -d "${CONFIGFS_ROOT}" ]] \
    || die "configfs not mounted at ${CONFIGFS_ROOT}; is sys-kernel-config.mount active?"

[[ -f "${CAMERA_STREAM_BIN}" ]] \
    || die "camera stream script not found at ${CAMERA_STREAM_BIN}; run the install script"

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
            die "no USB device controller found under /sys/class/udc after ${UDC_WAIT_SECS} seconds"
        fi
        sleep 1
    done
}

# ── module loading ────────────────────────────────────────────────────────────

log "Loading kernel modules"
modprobe dwc2         2>/dev/null || warn "dwc2 already loaded or unavailable"
modprobe libcomposite
modprobe usb_f_ncm    2>/dev/null || warn "usb_f_ncm not available (CDC NCM may be built-in)"

# ── gadget teardown (idempotent) ──────────────────────────────────────────────

if [[ -d "${GADGET_DIR}" ]]; then
    warn "Stale gadget directory found; removing before setup"
    teardown_gadget "${GADGET_DIR}"
fi

# ── UDC discovery ─────────────────────────────────────────────────────────────

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

mkdir -p "${GADGET_DIR}/configs/c.1/strings/0x409"
echo "${USB_CONFIGURATION}" > "${GADGET_DIR}/configs/c.1/strings/0x409/configuration"
echo "${MAX_POWER_MA}"      > "${GADGET_DIR}/configs/c.1/MaxPower"

# ── CDC NCM function ──────────────────────────────────────────────────────────

log "Creating CDC NCM function"
NCM="${GADGET_DIR}/functions/ncm.usb0"
mkdir -p "${NCM}"
echo "${NCM_DEV_ADDR}"  > "${NCM}/dev_addr"
echo "${NCM_HOST_ADDR}" > "${NCM}/host_addr"

ln -s "${NCM}" "${GADGET_DIR}/configs/c.1/ncm.usb0"

# ── bind to UDC ───────────────────────────────────────────────────────────────

log "Binding gadget to UDC ${UDC_NAME}"
echo "${UDC_NAME}" > "${GADGET_DIR}/UDC"

# ── configure USB network interface ───────────────────────────────────────────

log "Waiting for usb0 interface..."
for i in $(seq 1 10); do
    if ip link show usb0 &>/dev/null; then
        ip addr add "${NCM_PI_IP}/${NCM_PREFIX}" dev usb0 2>/dev/null || true
        ip link set usb0 up
        log "USB network interface usb0 up at ${NCM_PI_IP}"
        break
    fi
    sleep 0.5
done
if ! ip link show usb0 &>/dev/null; then
    warn "usb0 did not appear — CDC NCM may not be supported by the host kernel"
fi

# ── start camera stream ───────────────────────────────────────────────────────

log "Starting camera MJPEG stream at http://${NCM_PI_IP}:8080/stream.mjpg"
exec python3 -u "${CAMERA_STREAM_BIN}"
