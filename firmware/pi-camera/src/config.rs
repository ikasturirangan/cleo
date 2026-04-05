use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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
                "# Optional: pin a specific UDC name if your platform exposes more than one\n",
                "# SLITCAM_UDC_NAME=20980000.usb\n"
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

#[cfg(test)]
mod tests {
    use super::parse_u16;

    #[test]
    fn parses_hex_values() {
        assert_eq!(parse_u16("0x0525", "X").unwrap(), 0x0525);
        assert_eq!(parse_u16("0XA4A2", "X").unwrap(), 0xa4a2);
    }

    #[test]
    fn parses_decimal_values() {
        assert_eq!(parse_u16("500", "X").unwrap(), 500);
    }
}
