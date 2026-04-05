use std::env;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Settings {
    /// V4L2 device node for the Pi camera, e.g. `/dev/video0`.
    pub video_device: PathBuf,
    /// Address the HTTP API server will bind to, e.g. `0.0.0.0:8080`.
    pub api_bind: String,
    /// Linux I2C bus device for DLP2000, e.g. `/dev/i2c-2`.
    pub i2c_bus: PathBuf,
    /// 7-bit I2C address of the DLP2000 DLPC2607 controller.
    pub dlp_i2c_addr: u8,
    /// UART device for TMC2209, e.g. `/dev/ttyS1`.
    pub uart_device: PathBuf,
    /// UART baud rate for TMC2209 (typically 115200).
    pub uart_baud: u32,
    /// USB vendor ID of the Pi camera gadget (used for device discovery).
    pub camera_usb_vendor_id: u16,
    /// USB product ID of the Pi camera gadget.
    pub camera_usb_product_id: u16,
    /// How long to wait for the Pi camera to appear on USB.
    pub camera_wait_timeout: Duration,
    /// Where calibration data is persisted on disk.
    pub calibration_path: PathBuf,
}

impl Settings {
    pub fn from_env() -> Result<Self, String> {
        let settings = Self {
            video_device: PathBuf::from(env_or_default(
                "SLITCAM_VIDEO_DEVICE",
                "/dev/video0",
            )),
            api_bind: env_or_default("SLITCAM_API_BIND", "0.0.0.0:8080"),
            i2c_bus: PathBuf::from(env_or_default("SLITCAM_I2C_BUS", "/dev/i2c-2")),
            dlp_i2c_addr: parse_u8(
                &env_or_default("SLITCAM_DLP_I2C_ADDR", "0x1b"),
                "SLITCAM_DLP_I2C_ADDR",
            )?,
            uart_device: PathBuf::from(env_or_default(
                "SLITCAM_UART_DEVICE",
                "/dev/ttyS1",
            )),
            uart_baud: parse_u32(
                &env_or_default("SLITCAM_UART_BAUD", "115200"),
                "SLITCAM_UART_BAUD",
            )?,
            camera_usb_vendor_id: parse_u16(
                &env_or_default("SLITCAM_CAMERA_USB_VENDOR_ID", "0x0525"),
                "SLITCAM_CAMERA_USB_VENDOR_ID",
            )?,
            camera_usb_product_id: parse_u16(
                &env_or_default("SLITCAM_CAMERA_USB_PRODUCT_ID", "0xa4a2"),
                "SLITCAM_CAMERA_USB_PRODUCT_ID",
            )?,
            camera_wait_timeout: Duration::from_secs(parse_u64(
                &env_or_default("SLITCAM_CAMERA_WAIT_SECS", "30"),
                "SLITCAM_CAMERA_WAIT_SECS",
            )?),
            calibration_path: PathBuf::from(env_or_default(
                "SLITCAM_CALIBRATION_PATH",
                "/etc/slitcam/calibration.json",
            )),
        };
        settings.validate()?;
        Ok(settings)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.api_bind.trim().is_empty() {
            return Err("SLITCAM_API_BIND must not be empty".to_string());
        }
        if self.uart_baud == 0 {
            return Err("SLITCAM_UART_BAUD must be greater than zero".to_string());
        }
        if self.camera_wait_timeout.is_zero() {
            return Err("SLITCAM_CAMERA_WAIT_SECS must be greater than zero".to_string());
        }
        Ok(())
    }

    pub fn env_template(&self) -> String {
        format!(
            concat!(
                "# SlitCam BeagleBone controller runtime configuration\n",
                "SLITCAM_VIDEO_DEVICE={video_device}\n",
                "SLITCAM_API_BIND={api_bind}\n",
                "SLITCAM_I2C_BUS={i2c_bus}\n",
                "SLITCAM_DLP_I2C_ADDR=0x{dlp_addr:02x}\n",
                "SLITCAM_UART_DEVICE={uart_device}\n",
                "SLITCAM_UART_BAUD={uart_baud}\n",
                "SLITCAM_CAMERA_USB_VENDOR_ID=0x{vendor:04x}\n",
                "SLITCAM_CAMERA_USB_PRODUCT_ID=0x{product:04x}\n",
                "SLITCAM_CAMERA_WAIT_SECS={camera_wait}\n",
                "SLITCAM_CALIBRATION_PATH={cal_path}\n",
            ),
            video_device = self.video_device.display(),
            api_bind = self.api_bind,
            i2c_bus = self.i2c_bus.display(),
            dlp_addr = self.dlp_i2c_addr,
            uart_device = self.uart_device.display(),
            uart_baud = self.uart_baud,
            vendor = self.camera_usb_vendor_id,
            product = self.camera_usb_product_id,
            camera_wait = self.camera_wait_timeout.as_secs(),
            cal_path = self.calibration_path.display(),
        )
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn env_or_default(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn parse_u8(value: &str, key: &str) -> Result<u8, String> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        u8::from_str_radix(hex, 16).map_err(|_| format!("{key} must be a valid hex byte like 0x1b"))
    } else {
        trimmed
            .parse::<u8>()
            .map_err(|_| format!("{key} must be a valid integer"))
    }
}

fn parse_u16(value: &str, key: &str) -> Result<u16, String> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16)
            .map_err(|_| format!("{key} must be a valid hex value like 0x0525"))
    } else {
        trimmed
            .parse::<u16>()
            .map_err(|_| format!("{key} must be a valid integer"))
    }
}

fn parse_u32(value: &str, key: &str) -> Result<u32, String> {
    value
        .trim()
        .parse::<u32>()
        .map_err(|_| format!("{key} must be a valid integer"))
}

fn parse_u64(value: &str, key: &str) -> Result<u64, String> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|_| format!("{key} must be a valid integer"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_u8() {
        assert_eq!(parse_u8("0x1b", "X").unwrap(), 0x1b);
        assert_eq!(parse_u8("0X1B", "X").unwrap(), 0x1b);
    }

    #[test]
    fn parses_hex_u16() {
        assert_eq!(parse_u16("0x0525", "X").unwrap(), 0x0525);
    }

    #[test]
    fn parses_decimal() {
        assert_eq!(parse_u32("115200", "X").unwrap(), 115200);
        assert_eq!(parse_u64("30", "X").unwrap(), 30);
    }
}
