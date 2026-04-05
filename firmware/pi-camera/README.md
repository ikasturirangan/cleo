# Pi Camera Firmware

Production-oriented Raspberry Pi Zero 2 W firmware for the slit lamp camera module.

## Responsibilities

- validate camera availability
- configure the Pi Zero 2 W OTG USB port for UVC gadget mode
- create and clean up the USB gadget through `configfs`
- launch `uvc-gadget` against the Raspberry Pi Camera Module 3
- start automatically on boot through `systemd`

## Directory Contents

- `src/`: Rust runtime
- `deploy/pi-camera.env`: default environment configuration
- `deploy/slitcam-pi-camera.service`: `systemd` unit
- `scripts/install_from_checkout.sh`: build and install on a Raspberry Pi

## Build and Install on Pi

From a sparse checkout or full checkout:

```bash
cd firmware/pi-camera
sudo ./scripts/install_from_checkout.sh
```

Then reboot:

```bash
sudo reboot
```

After reboot, connect the Pi Zero 2 W to the BeagleBone Black or a Mac using the `USB` data port. The device should enumerate as `SlitCam Pi Camera`.

## Default Runtime Commands

The installed binary is:

```bash
/usr/local/bin/slitcam-pi-camera
```

Supported commands:

```bash
slitcam-pi-camera preflight
slitcam-pi-camera run
slitcam-pi-camera cleanup
slitcam-pi-camera print-env
```

## Runtime Configuration

The installer places the active configuration at:

```bash
/etc/slitcam/pi-camera.env
```

The shipped defaults are conservative and should work for a single-camera Pi Zero 2 W setup.

## Boot Configuration

The installer updates the Pi boot files so gadget mode is available at boot:

- `dtoverlay=dwc2,dr_mode=peripheral` in `config.txt`
- `modules-load=dwc2` in `cmdline.txt`

If the service logs `no USB device controller found under /sys/class/udc`, the boot configuration is the first thing to check.

## Service Logs

```bash
sudo systemctl status slitcam-pi-camera.service
sudo journalctl -u slitcam-pi-camera.service -b --no-pager
```
