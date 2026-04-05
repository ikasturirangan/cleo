# Slit Lamp Control System

This repository is structured so the Raspberry Pi camera firmware can be cloned and built independently from the future web application.

## Layout

- `firmware/pi-camera`: Raspberry Pi Zero 2 W camera firmware and deployment assets
- `webapp`: reserved for the UI and backend application that will run elsewhere

The Pi should only need `firmware/pi-camera`.

## Raspberry Pi Firmware Checkout

When this repository is pushed to Git, use sparse checkout on the Pi so the web app is not downloaded into the working tree:

```bash
git clone --filter=blob:none --sparse <repo-url> slitlamp
cd slitlamp
git sparse-checkout set firmware/pi-camera README.md
cd firmware/pi-camera
sudo ./scripts/install_from_checkout.sh
```

That checks out the root README and the firmware subtree only.

## Raspberry Pi Firmware Purpose

The Pi Zero 2 W acts as a camera appliance:

- captures video from the Raspberry Pi Camera Module 3
- exposes the camera to the BeagleBone Black as a USB UVC webcam
- boots directly into the webcam service through `systemd`

## Notes

- The Pi-side firmware is implemented in Rust for the control/runtime layer, with shell scripts only for installation and OS integration.
- The actual UVC video transport still relies on `uvc-gadget`, which is currently the practical userspace bridge for Raspberry Pi camera to USB UVC gadget mode.
- The installer forces `dtoverlay=dwc2,dr_mode=peripheral` and adds `modules-load=dwc2` to the Pi boot command line so `/sys/class/udc` is present on Pi Zero 2 W.
