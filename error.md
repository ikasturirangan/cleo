# USB Webcam Enumeration Error

## Summary

The Raspberry Pi Zero 2 W camera firmware installs and starts, but the Pi does not consistently detect a USB host on the OTG/data port. Because of that, the UVC webcam gadget does not bind and the host computer does not see a webcam device.

## Hardware Context

- Raspberry Pi Zero 2 W
- Raspberry Pi Camera Module 3 (`imx708`)
- Host tested on:
  - macOS
  - Windows

## Expected Behavior

When the host is connected to the Pi Zero 2 W port labeled `USB`:

- `/sys/class/udc` on the Pi should become non-empty
- the `slitcam-pi-camera.service` runtime should bind the gadget
- `uvc-gadget` should start
- the host should enumerate the Pi as a UVC webcam

## Actual Behavior

- `slitcam-pi-camera.service` starts
- the camera is detected correctly
- `/sys/class/udc` remains empty after connecting the host
- after waiting for host detection, the service exits with:

```text
no USB device controller found under /sys/class/udc after waiting 60 seconds; connect the USB host to the Pi Zero 2 W data port
```

- macOS does not show the Pi under `system_profiler SPUSBDataType`
- QuickTime Player and Google Meet only show the built-in camera
- Windows testing showed the same Pi-side behavior

## What Has Been Confirmed

- Camera detection works:

```bash
rpicam-hello --list-cameras
```

- The camera appears as:

```text
0 : imx708 [4608x2592 10-bit RGGB]
```

- Boot configuration was corrected so the Pi Zero 2 W uses gadget-capable USB mode:
  - `dtoverlay=dwc2,dr_mode=peripheral`
  - `modules-load=dwc2`

- A valid UDC has appeared in some earlier tests after correcting the config:

```text
3f980000.usb
```

- The firmware repo now includes:
  - a Rust runtime for gadget setup
  - a wait-for-host flow instead of immediate preflight failure
  - a prebuilt Linux ARM64 binary so the Pi does not need to compile Rust locally

## What Has Been Tried

- Corrected `config.txt` overlay placement into an active `[all]` section
- Added `modules-load=dwc2` to `cmdline.txt`
- Reinstalled firmware from the GitHub repo
- Rebooted and retested
- Tried multiple USB cables
- Verified use of the Pi Zero 2 W `USB` port instead of `PWR IN`
- Tested with macOS and Windows hosts
- Added runtime waiting for host connection before failing
- Cross-compiled the firmware on macOS and committed a prebuilt Linux ARM64 binary

## Current Likely Root Cause

The remaining blocker appears to be in the physical USB host/data path rather than the camera stack or firmware build:

- charge-only or marginal cable
- host adapter/hub path issue
- unstable host-provided power
- Pi Zero 2 W USB data path or board-level issue

## Recommended Next Diagnostics

1. Power the Pi separately through `PWR IN`.
2. Connect the host only to the Pi port labeled `USB`.
3. Avoid hubs/docks/adapters if possible.
4. Keep this running on the Pi while reconnecting the host:

```bash
watch -n 1 ls /sys/class/udc
```

5. Check for power instability:

```bash
vcgencmd get_throttled
dmesg -T | grep -i voltage
```

6. As a control test, temporarily disable the webcam service and try the official Raspberry Pi `rpi-usb-gadget` Ethernet gadget package:

```bash
sudo systemctl disable --now slitcam-pi-camera.service
sudo apt install -y rpi-usb-gadget
sudo rpi-usb-gadget on
sudo reboot
```

If `rpi-usb-gadget` also fails to enumerate, the issue is almost certainly hardware/power/USB-path related rather than the webcam runtime.

## Relevant Repo State

- Prebuilt binary workflow added for Pi installs
- Latest Pi firmware changes include:
  - host-wait behavior
  - low-memory Pi build profile
  - prebuilt `linux-aarch64` binary support
