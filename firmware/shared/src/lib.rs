use serde::{Deserialize, Serialize};

// USB identifiers for the Pi camera gadget — must match pi-camera deploy/pi-camera.env.
pub const PI_CAMERA_USB_VENDOR_ID: u16 = 0x0525;
pub const PI_CAMERA_USB_PRODUCT_ID: u16 = 0xa4a2;

// ── Slit projection ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlitConfig {
    /// Slit width in micrometers at the image plane.
    pub width_um: f32,
    /// Clockwise rotation angle in degrees.
    pub angle_deg: f32,
    /// Intensity (0 = off, 255 = full brightness).
    pub brightness: u8,
    /// Horizontal offset from centre in display pixels.
    pub offset_x_px: i16,
    /// Vertical offset from centre in display pixels.
    pub offset_y_px: i16,
}

impl Default for SlitConfig {
    fn default() -> Self {
        Self {
            width_um: 200.0,
            angle_deg: 0.0,
            brightness: 128,
            offset_x_px: 0,
            offset_y_px: 0,
        }
    }
}

// ── Motion ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MotionState {
    /// Current position in full steps from the home position.
    pub position_steps: i32,
    /// True once a successful homing sequence has completed.
    pub homed: bool,
}

// ── Camera ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CameraInfo {
    pub connected: bool,
    /// Path to the V4L2 device node, e.g. `/dev/video0`.
    pub device_path: String,
    pub capture_width: u32,
    pub capture_height: u32,
}

impl Default for CameraInfo {
    fn default() -> Self {
        Self {
            connected: false,
            device_path: String::new(),
            capture_width: 0,
            capture_height: 0,
        }
    }
}

// ── Device state (full snapshot) ─────────────────────────────────────────────

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DeviceState {
    pub camera: CameraInfo,
    pub slit: SlitConfig,
    pub motion: MotionState,
    pub dlp_ready: bool,
    /// Active fault strings; empty when healthy.
    pub errors: Vec<String>,
}

// ── Control protocol ─────────────────────────────────────────────────────────

/// Commands sent from the web application to the BeagleBone controller.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlCommand {
    /// Update slit projection parameters.
    SetSlit(SlitConfig),
    /// Move the focus stage by a relative number of steps (positive = forward).
    MoveFocus { steps: i32 },
    /// Run the homing sequence and zero the position counter.
    HomeFocus,
    /// Change the camera capture resolution.
    SetCaptureFormat { width: u32, height: u32 },
    /// Request a full device state snapshot.
    GetState,
    /// Connectivity check; always returns `Ok`.
    Ping,
}

/// Responses from the BeagleBone controller.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandResponse {
    Ok,
    State(DeviceState),
    Error { message: String },
}

impl CommandResponse {
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
        }
    }
}

// ── Calibration ──────────────────────────────────────────────────────────────

/// Persisted calibration data saved under `SLITCAM_CALIBRATION_PATH`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalibrationData {
    /// Camera sensor pixels per micrometer at the image plane.
    pub pixels_per_um: f32,
    /// The slit configuration used when this calibration was captured.
    pub slit_reference: SlitConfig,
    /// Unix timestamp (seconds) when the calibration was saved.
    pub timestamp_unix: u64,
}
