#!/usr/bin/env bash
# Cross-compile slitcam-bbb-controller for the BeagleBone Black (armv7) on
# macOS or Linux and write the result to prebuilt/linux-armv7/.
#
# Requirements:
#   rustup target add armv7-unknown-linux-musleabihf
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
RUSTUP_CARGO="${HOME}/.cargo/bin/cargo"
RUSTUP_RUSTUP="${HOME}/.cargo/bin/rustup"
TARGET_TRIPLE="armv7-unknown-linux-musleabihf"
PROFILE="bbb-release"
OUTPUT_DIR="${PROJECT_DIR}/prebuilt/linux-armv7"
OUTPUT_BIN="${OUTPUT_DIR}/slitcam-bbb-controller"

if [[ ! -x "${RUSTUP_CARGO}" || ! -x "${RUSTUP_RUSTUP}" ]]; then
  echo "Expected rustup-managed cargo under ~/.cargo/bin" >&2
  exit 1
fi

if ! "${RUSTUP_RUSTUP}" target list --installed | grep -qx "${TARGET_TRIPLE}"; then
  echo "Missing Rust target ${TARGET_TRIPLE}. Install it with:" >&2
  echo "  rustup target add ${TARGET_TRIPLE}" >&2
  exit 1
fi

mkdir -p "${OUTPUT_DIR}"

(
  cd "${PROJECT_DIR}"
  RUSTC="${HOME}/.cargo/bin/rustc" \
    CARGO_TARGET_ARMV7_UNKNOWN_LINUX_MUSLEABIHF_LINKER=rust-lld \
    "${RUSTUP_CARGO}" build \
      --profile "${PROFILE}" \
      --target "${TARGET_TRIPLE}"
)

install -m 0755 \
  "${PROJECT_DIR}/target/${TARGET_TRIPLE}/${PROFILE}/slitcam-bbb-controller" \
  "${OUTPUT_BIN}"

echo "Wrote ${OUTPUT_BIN}"
