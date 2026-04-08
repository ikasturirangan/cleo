#!/usr/bin/env bash
# UVC + CDC NCM composite gadget for Raspberry Pi Zero 2 W.
#
# Exposes two USB functions:
#   uvc.0  — webcam (UVC)
#   ncm.usb0 — USB ethernet (CDC NCM) at 192.168.7.1
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
USB_CONFIGURATION="${SLITCAM_USB_CONFIGURATION:-UVC+NCM}"
MAX_POWER_MA="${SLITCAM_USB_MAX_POWER_MA:-500}"
CAMERA_ID="${SLITCAM_CAMERA_ID:-0}"
UVC_GADGET_BIN="${SLITCAM_UVC_GADGET_BIN:-/usr/local/bin/uvc-gadget}"
UDC_WAIT_SECS="${SLITCAM_UDC_WAIT_SECS:-60}"
UVC_RESOLUTION="${SLITCAM_UVC_RESOLUTION:-640x480}"
UVC_FRAMERATE="${SLITCAM_UVC_FRAMERATE:-30}"

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
#
# configfs does NOT support rm -rf — the kernel requires an exact teardown
# order: remove symlinks first, then rmdir leaf dirs, then parents.

teardown_gadget() {
    local gdir="$1"
    [[ -d "${gdir}" ]] || return 0

    local f="${gdir}/functions/uvc.0"

    # 1. Bring down USB network interface before unbinding.
    ip link set usb0 down   2>/dev/null || true
    ip addr flush dev usb0  2>/dev/null || true

    # 2. Unbind from UDC so the host sees device disconnect.
    if [[ -f "${gdir}/UDC" ]]; then
        echo "" > "${gdir}/UDC" 2>/dev/null || true
        sleep 0.1
    fi

    # 3. Remove config symlinks (UVC and NCM).
    rm -f "${gdir}/configs/c.1/uvc.0"   2>/dev/null || true
    rm -f "${gdir}/configs/c.1/ncm.usb0" 2>/dev/null || true

    # 4. Tear down UVC function (symlinks → leaf dirs → parent dirs).
    if [[ -d "${f}" ]]; then
        rm -f "${f}/streaming/class/fs/h" 2>/dev/null || true
        rm -f "${f}/streaming/class/hs/h" 2>/dev/null || true
        rm -f "${f}/streaming/class/ss/h" 2>/dev/null || true
        rm -f "${f}/control/class/fs/h"   2>/dev/null || true
        rm -f "${f}/control/class/ss/h"   2>/dev/null || true
        rm -f "${f}/streaming/header/h/u" 2>/dev/null || true
        rm -f "${f}/streaming/header/h/m" 2>/dev/null || true

        rmdir "${f}/streaming/class/fs" 2>/dev/null || true
        rmdir "${f}/streaming/class/hs" 2>/dev/null || true
        rmdir "${f}/streaming/class/ss" 2>/dev/null || true
        rmdir "${f}/streaming/class"    2>/dev/null || true
        rmdir "${f}/control/class/fs"   2>/dev/null || true
        rmdir "${f}/control/class/ss"   2>/dev/null || true
        rmdir "${f}/control/class"      2>/dev/null || true
        rmdir "${f}/streaming/header/h" 2>/dev/null || true
        rmdir "${f}/streaming/header"   2>/dev/null || true
        rmdir "${f}/control/header/h"   2>/dev/null || true
        rmdir "${f}/control/header"     2>/dev/null || true

        for frame_dir in "${f}/streaming/uncompressed/u"/*/; do
            rmdir "${frame_dir}" 2>/dev/null || true
        done
        rmdir "${f}/streaming/uncompressed/u" 2>/dev/null || true
        rmdir "${f}/streaming/uncompressed"   2>/dev/null || true

        for frame_dir in "${f}/streaming/mjpeg/m"/*/; do
            rmdir "${frame_dir}" 2>/dev/null || true
        done
        rmdir "${f}/streaming/mjpeg/m" 2>/dev/null || true
        rmdir "${f}/streaming/mjpeg"   2>/dev/null || true

        rmdir "${f}/streaming" 2>/dev/null || true
        rmdir "${f}/control"   2>/dev/null || true
        rmdir "${f}"           2>/dev/null || true
    fi

    # 5. Remove NCM function dir (kernel-managed, just rmdir).
    rmdir "${gdir}/functions/ncm.usb0" 2>/dev/null || true

    # 6. Remove config strings then config.
    rmdir "${gdir}/configs/c.1/strings/0x409" 2>/dev/null || true
    rmdir "${gdir}/configs/c.1/strings"       2>/dev/null || true
    rmdir "${gdir}/configs/c.1"               2>/dev/null || true
    rmdir "${gdir}/configs"                   2>/dev/null || true

    # 7. Remove gadget strings then gadget root.
    rmdir "${gdir}/strings/0x409" 2>/dev/null || true
    rmdir "${gdir}/strings"       2>/dev/null || true
    rmdir "${gdir}"               2>/dev/null || true
}

cleanup() {
    log "Cleaning up gadget ${GADGET_NAME}"
    teardown_gadget "${GADGET_DIR}"
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

add_frame() {
    local fmt="$1" name="$2" w="$3" h="$4"
    shift 4
    local fdir="${F}/streaming/${fmt}/${name}/${h}p"
    mkdir -p "${fdir}"
    echo "${w}"             > "${fdir}/wWidth"
    echo "${h}"             > "${fdir}/wHeight"
    echo $(( w * h * 2 ))  > "${fdir}/dwMaxVideoFrameBufferSize"
    printf '%s\n' "$@"     > "${fdir}/dwFrameInterval"
}

add_frame uncompressed u  640  480  333333 416667 500000 666666 1000000 1333333 2000000
add_frame uncompressed u 1280  720  1000000 1333333 2000000
add_frame uncompressed u 1920 1080  2000000

add_frame mjpeg m  640  480  333333 416667 500000 666666 1000000 1333333 2000000
add_frame mjpeg m 1280  720  333333 416667 500000 666666 1000000 1333333 2000000
add_frame mjpeg m 1920 1080  333333 416667 500000 666666 1000000 1333333 2000000

ln -s "${F}/streaming/uncompressed/u" "${F}/streaming/header/h/u"
ln -s "${F}/streaming/mjpeg/m"        "${F}/streaming/header/h/m"

ln -s "${F}/streaming/header/h" "${F}/streaming/class/fs/h"
ln -s "${F}/streaming/header/h" "${F}/streaming/class/hs/h"
ln -s "${F}/streaming/header/h" "${F}/streaming/class/ss/h"

ln -s "${F}/control/header/h" "${F}/control/class/fs/h"
ln -s "${F}/control/header/h" "${F}/control/class/ss/h"

echo "2048" > "${F}/streaming_maxpacket"

ln -s "${F}" "${GADGET_DIR}/configs/c.1/uvc.0"

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

# kbingham/uvc-gadget is V4L2-only; route camera through libcamera's v4l2-compat
# shim so it can access the Pi camera (imx708) which only speaks libcamera.
V4L2_COMPAT="$(find /usr/lib -name "v4l2-compat.so" 2>/dev/null | head -1)"
[[ -n "${V4L2_COMPAT}" ]] \
    || die "libcamera v4l2-compat.so not found; run: sudo apt-get install -y libcamera-v4l2"

log "Starting uvc-gadget (camera /dev/video${CAMERA_ID} via ${V4L2_COMPAT})"
exec env LD_PRELOAD="${V4L2_COMPAT}" \
    "${UVC_GADGET_BIN}" -c "/dev/video${CAMERA_ID}" uvc.0
