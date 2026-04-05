use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::Settings;
use crate::logging;

pub struct Camera {
    pub device_path: PathBuf,
}

impl Camera {
    /// Search `/sys/bus/usb/devices` for a device matching the Pi's USB IDs
    /// and return its `/dev/videoN` node, or `None` if not yet visible.
    pub fn find(settings: &Settings) -> Result<Option<PathBuf>, String> {
        let vendor_str = format!("{:04x}", settings.camera_usb_vendor_id);
        let product_str = format!("{:04x}", settings.camera_usb_product_id);

        let usb_root = Path::new("/sys/bus/usb/devices");
        let entries =
            fs::read_dir(usb_root).map_err(|e| format!("read /sys/bus/usb/devices: {e}"))?;

        for entry in entries.flatten() {
            let dev = entry.path();
            let vendor = read_trimmed(dev.join("idVendor"));
            let product = read_trimmed(dev.join("idProduct"));

            if vendor.as_deref() != Some(vendor_str.as_str())
                || product.as_deref() != Some(product_str.as_str())
            {
                continue;
            }

            // Found the Pi USB device — walk its child interfaces for a
            // video4linux node.
            if let Some(node) = find_video_node(&dev) {
                return Ok(Some(node));
            }
        }

        Ok(None)
    }

    /// Block until the Pi camera appears on USB or the timeout expires.
    pub fn wait_for_device(settings: &Settings) -> Result<PathBuf, String> {
        logging::info("Waiting for Pi camera on USB...");
        let deadline = Instant::now() + settings.camera_wait_timeout;

        loop {
            if let Some(path) = Self::find(settings)? {
                return Ok(path);
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "Pi camera not found after {} seconds; verify the Pi is connected and \
                     enumerated as a UVC device",
                    settings.camera_wait_timeout.as_secs()
                ));
            }
            thread::sleep(Duration::from_secs(2));
        }
    }

    /// Open a V4L2 device node, confirm it exists, and return a Camera handle.
    pub fn open(device_path: PathBuf) -> Result<Self, String> {
        if !device_path.exists() {
            return Err(format!(
                "camera device {} does not exist",
                device_path.display()
            ));
        }
        logging::info(format!("Camera device: {}", device_path.display()));
        Ok(Self { device_path })
    }

    /// Set up V4L2 buffer queues and begin streaming.
    ///
    /// TODO: open the fd, VIDIOC_S_FMT (MJPEG/YUYV), VIDIOC_REQBUFS (MMAP),
    ///       VIDIOC_QBUF for each buffer, VIDIOC_STREAMON.
    pub fn start_capture(&self) -> Result<(), String> {
        logging::warn("camera V4L2 capture not yet implemented");
        Ok(())
    }

    /// Dequeue the next frame buffer and return a reference to its data.
    ///
    /// TODO: VIDIOC_DQBUF, copy/alias data, VIDIOC_QBUF to recycle.
    pub fn next_frame(&self) -> Result<Vec<u8>, String> {
        Err("camera V4L2 frame capture not yet implemented".to_string())
    }
}

// ── sysfs helpers ─────────────────────────────────────────────────────────────

/// Walk child directories of a USB device looking for a `video4linux/videoN`
/// entry and return the corresponding `/dev/videoN` path.
fn find_video_node(usb_dev: &Path) -> Option<PathBuf> {
    for child in fs::read_dir(usb_dev).ok()?.flatten() {
        let v4l = child.path().join("video4linux");
        if v4l.is_dir() {
            if let Some(node) = first_video_node(&v4l) {
                return Some(node);
            }
        }
    }
    None
}

fn first_video_node(v4l_dir: &Path) -> Option<PathBuf> {
    for entry in fs::read_dir(v4l_dir).ok()?.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("video") {
            return Some(PathBuf::from("/dev").join(name.as_ref()));
        }
    }
    None
}

fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
