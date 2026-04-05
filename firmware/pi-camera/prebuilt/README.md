# Prebuilt Firmware Binaries

If you do not want to compile Rust on the Raspberry Pi Zero 2 W, place a prebuilt Linux ARM64 binary here:

```text
prebuilt/linux-aarch64/slitcam-pi-camera
```

The installer will detect that file automatically and install it instead of running `cargo build` on the Pi.

You can also override the path explicitly:

```bash
sudo SLITCAM_PREBUILT_BIN=/path/to/slitcam-pi-camera ./scripts/install_from_checkout.sh
```

Requirements for the prebuilt binary:

- target architecture: `aarch64`
- operating system: Linux
- executable name: `slitcam-pi-camera`

If no prebuilt binary is present, the installer falls back to a low-memory on-device build using the `pi-release` Cargo profile and `1` build job by default.
