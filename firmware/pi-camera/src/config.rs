use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Settings {
    pub configfs_root: PathBuf,
    pub gadget_name: String,
    pub usb_vendor_id: u16,
    pub usb_product_id: u16,
    pub usb_serial: String,
    pub usb_manufacturer: String,
    pub usb_product: String,
    pub usb_configuration: String,
    pub max_power_ma: u16,
    pub camera_id: String,
    pub uvc_gadget_bin: PathBuf,
    pub preferred_udc: Option<String>,
    pub udc_wait_timeout: Duration,
    /// Resolution passed to uvc-gadget, e.g. `1280x720`.
    pub uvc_resolution: String,
    /// Target frame rate passed to uvc-gadget.
    pub uvc_framerate: u32,
    // ── TMC2209 motion ────────────────────────────────────────────────────────
    /// UART device connected to TMC2209 PDN_UART, e.g. `/dev/ttyAMA0`.
    pub uart_device: PathBuf,
    /// UART baud rate for TMC2209 (typically 115200).
    pub uart_baud: u32,
    /// Seconds to wait for UART device to appear before failing preflight.
    pub uart_wait_secs: u64,
    /// StallGuard threshold (0–255). Higher = less sensitive.
    pub stallguard_threshold: u8,
    /// Full steps to back off after a stall is detected during homing.
    pub home_backoff_steps: u32,
    /// Run current (0–31 RMS scale).
    pub motor_run_current: u8,
    /// Hold current (0–31 RMS scale).
    pub motor_hold_current: u8,
}

impl Settings {
    pub fn from_env() -> Result<Self, String> {
        let hostname = detect_hostname();

        let settings = Self {
            configfs_root: PathBuf::from(env_or_default(
                "SLITCAM_CONFIGFS_ROOT",
                "/sys/kernel/config",
            )),
            gadget_name: env_or_default("SLITCAM_USB_GADGET_NAME", "slitcam0"),
            usb_vendor_id: parse_u16(
                &env_or_default("SLITCAM_USB_VENDOR_ID", "0x0525"),
                "SLITCAM_USB_VENDOR_ID",
            )?,
            usb_product_id: parse_u16(
                &env_or_default("SLITCAM_USB_PRODUCT_ID", "0xa4a2"),
                "SLITCAM_USB_PRODUCT_ID",
            )?,
            usb_serial: env_or_default("SLITCAM_USB_SERIAL", "SLITCAM-0001"),
            usb_manufacturer: env_or_default("SLITCAM_USB_MANUFACTURER", &hostname),
            usb_product: env_or_default("SLITCAM_USB_PRODUCT", "SlitCam Pi Camera"),
            usb_configuration: env_or_default("SLITCAM_USB_CONFIGURATION", "UVC"),
            max_power_ma: parse_u16(
                &env_or_default("SLITCAM_USB_MAX_POWER_MA", "500"),
                "SLITCAM_USB_MAX_POWER_MA",
            )?,
            camera_id: env_or_default("SLITCAM_CAMERA_ID", "0"),
            uvc_gadget_bin: PathBuf::from(env_or_default(
                "SLITCAM_UVC_GADGET_BIN",
                "/usr/local/bin/uvc-gadget",
            )),
            preferred_udc: optional_env("SLITCAM_UDC_NAME"),
            uvc_resolution: env_or_default("SLITCAM_UVC_RESOLUTION", "640x480"),
            uvc_framerate: parse_u32(
                &env_or_default("SLITCAM_UVC_FRAMERATE", "30"),
                "SLITCAM_UVC_FRAMERATE",
            )?,
            udc_wait_timeout: Duration::from_secs(parse_u64(
                &env_or_default("SLITCAM_UDC_WAIT_SECS", "60"),
                "SLITCAM_UDC_WAIT_SECS",
            )?),
            uart_device: PathBuf::from(env_or_default(
                "SLITCAM_UART_DEVICE",
                "/dev/ttyAMA0",
            )),
            uart_baud: parse_u32(
                &env_or_default("SLITCAM_UART_BAUD", "115200"),
                "SLITCAM_UART_BAUD",
            )?,
            uart_wait_secs: parse_u64(
                &env_or_default("SLITCAM_UART_WAIT_SECS", "10"),
                "SLITCAM_UART_WAIT_SECS",
            )?,
            stallguard_threshold: parse_u8(
                &env_or_default("SLITCAM_STALLGUARD_THRESHOLD", "80"),
                "SLITCAM_STALLGUARD_THRESHOLD",
            )?,
            home_backoff_steps: parse_u32(
                &env_or_default("SLITCAM_HOME_BACKOFF_STEPS", "50"),
                "SLITCAM_HOME_BACKOFF_STEPS",
            )?,
            motor_run_current: parse_u8(
                &env_or_default("SLITCAM_MOTOR_RUN_CURRENT", "20"),
                "SLITCAM_MOTOR_RUN_CURRENT",
            )?,
            motor_hold_current: parse_u8(
                &env_or_default("SLITCAM_MOTOR_HOLD_CURRENT", "8"),
                "SLITCAM_MOTOR_HOLD_CURRENT",
            )?,
        };

        settings.validate()?;
        Ok(settings)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.gadget_name.trim().is_empty() {
            return Err("SLITCAM_USB_GADGET_NAME must not be empty".to_string());
        }
        if self.gadget_name.contains('/') {
            return Err("SLITCAM_USB_GADGET_NAME must not contain '/'".to_string());
        }
        if self.usb_serial.trim().is_empty() {
            return Err("SLITCAM_USB_SERIAL must not be empty".to_string());
        }
        if self.usb_manufacturer.trim().is_empty() {
            return Err("SLITCAM_USB_MANUFACTURER must not be empty".to_string());
        }
        if self.usb_product.trim().is_empty() {
            return Err("SLITCAM_USB_PRODUCT must not be empty".to_string());
        }
        if self.usb_configuration.trim().is_empty() {
            return Err("SLITCAM_USB_CONFIGURATION must not be empty".to_string());
        }
        if self.max_power_ma == 0 {
            return Err("SLITCAM_USB_MAX_POWER_MA must be greater than zero".to_string());
        }
        if self.camera_id.trim().is_empty() {
            return Err("SLITCAM_CAMERA_ID must not be empty".to_string());
        }
        if self.udc_wait_timeout.is_zero() {
            return Err("SLITCAM_UDC_WAIT_SECS must be greater than zero".to_string());
        }
        if self.uart_baud == 0 {
            return Err("SLITCAM_UART_BAUD must be greater than zero".to_string());
        }
        if self.motor_run_current > 31 {
            return Err("SLITCAM_MOTOR_RUN_CURRENT must be 0–31".to_string());
        }
        if self.motor_hold_current > 31 {
            return Err("SLITCAM_MOTOR_HOLD_CURRENT must be 0–31".to_string());
        }
        Ok(())
    }

    pub fn gadget_dir(&self) -> PathBuf {
        self.configfs_root
            .join("usb_gadget")
            .join(&self.gadget_name)
    }

    pub fn config_dir(&self) -> PathBuf {
        self.gadget_dir().join("configs").join("c.1")
    }

    pub fn function_dir(&self) -> PathBuf {
        self.gadget_dir()
            .join("functions")
            .join(self.uvc_function_name())
    }

    pub fn uvc_function_name(&self) -> &'static str {
        "uvc.0"
    }

    pub fn usb_vendor_id_hex(&self) -> String {
        format!("0x{:04x}", self.usb_vendor_id)
    }

    pub fn usb_product_id_hex(&self) -> String {
        format!("0x{:04x}", self.usb_product_id)
    }

    pub fn env_template(&self) -> String {
        format!(
            concat!(
                "# SlitCam Pi camera runtime configuration\n",
                "SLITCAM_CONFIGFS_ROOT={configfs_root}\n",
                "SLITCAM_USB_GADGET_NAME={gadget_name}\n",
                "SLITCAM_USB_VENDOR_ID={vendor_id}\n",
                "SLITCAM_USB_PRODUCT_ID={product_id}\n",
                "SLITCAM_USB_SERIAL={serial}\n",
                "SLITCAM_USB_MANUFACTURER={manufacturer}\n",
                "SLITCAM_USB_PRODUCT={product}\n",
                "SLITCAM_USB_CONFIGURATION={configuration}\n",
                "SLITCAM_USB_MAX_POWER_MA={max_power}\n",
                "SLITCAM_CAMERA_ID={camera_id}\n",
                "SLITCAM_UVC_GADGET_BIN={uvc_bin}\n",
                "SLITCAM_UDC_WAIT_SECS={udc_wait_secs}\n",
                "# Optional: pin a specific UDC name if your platform exposes more than one\n",
                "# SLITCAM_UDC_NAME=20980000.usb\n",
                "\n",
                "# TMC2209 motion control\n",
                "SLITCAM_UART_DEVICE={uart_device}\n",
                "SLITCAM_UART_BAUD={uart_baud}\n",
                "SLITCAM_UART_WAIT_SECS={uart_wait_secs}\n",
                "SLITCAM_STALLGUARD_THRESHOLD={stallguard_threshold}\n",
                "SLITCAM_HOME_BACKOFF_STEPS={home_backoff_steps}\n",
                "SLITCAM_MOTOR_RUN_CURRENT={motor_run_current}\n",
                "SLITCAM_MOTOR_HOLD_CURRENT={motor_hold_current}\n",
            ),
            configfs_root = self.configfs_root.display(),
            gadget_name = self.gadget_name,
            vendor_id = self.usb_vendor_id_hex(),
            product_id = self.usb_product_id_hex(),
            serial = self.usb_serial,
            manufacturer = self.usb_manufacturer,
            product = self.usb_product,
            configuration = self.usb_configuration,
            max_power = self.max_power_ma,
            camera_id = self.camera_id,
            uvc_bin = self.uvc_gadget_bin.display(),
            udc_wait_secs = self.udc_wait_timeout.as_secs(),
            uart_device = self.uart_device.display(),
            uart_baud = self.uart_baud,
            uart_wait_secs = self.uart_wait_secs,
            stallguard_threshold = self.stallguard_threshold,
            home_backoff_steps = self.home_backoff_steps,
            motor_run_current = self.motor_run_current,
            motor_hold_current = self.motor_hold_current,
        )
    }
}

fn env_or_default(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn optional_env(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn detect_hostname() -> String {
    read_trimmed("/proc/sys/kernel/hostname")
        .or_else(|| read_trimmed("/etc/hostname"))
        .unwrap_or_else(|| "slitcam-pi".to_string())
}

fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_u8(value: &str, key: &str) -> Result<u8, String> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        u8::from_str_radix(hex, 16).map_err(|_| format!("{key} must be a valid hex byte"))
    } else {
        trimmed
            .parse::<u8>()
            .map_err(|_| format!("{key} must be a valid integer (0–255)"))
    }
}

fn parse_u32(value: &str, key: &str) -> Result<u32, String> {
    value
        .trim()
        .parse::<u32>()
        .map_err(|_| format!("{key} must be a valid integer"))
}

fn parse_u16(value: &str, key: &str) -> Result<u16, String> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        u16::from_str_radix(hex, 16)
            .map_err(|_| format!("{key} must be a valid hex value like 0x0525"))
    } else {
        trimmed
            .parse::<u16>()
            .map_err(|_| format!("{key} must be a valid integer"))
    }
}

fn parse_u64(value: &str, key: &str) -> Result<u64, String> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|_| format!("{key} must be a valid integer"))
}

#[cfg(test)]
mod tests {
    use super::{parse_u16, parse_u64};

    #[test]
    fn parses_hex_values() {
        assert_eq!(parse_u16("0x0525", "X").unwrap(), 0x0525);
        assert_eq!(parse_u16("0XA4A2", "X").unwrap(), 0xa4a2);
    }

    #[test]
    fn parses_decimal_values() {
        assert_eq!(parse_u16("500", "X").unwrap(), 500);
    }

    #[test]
    fn parses_u64_values() {
        assert_eq!(parse_u64("60", "X").unwrap(), 60);
    }
}
