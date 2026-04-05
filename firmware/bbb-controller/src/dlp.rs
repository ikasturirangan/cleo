use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::io::AsRawFd;

use slitcam_shared::SlitConfig;

use crate::config::Settings;
use crate::logging;

// Linux I2C ioctl to select the slave address.
const I2C_SLAVE: libc::c_ulong = 0x0703;

// DLPC2607 command identifiers (two-byte big-endian prefix on each write).
// See DLPC2607 Programmer's Guide (DLPU030), Table 2.
const CMD_DEVICE_ID: u16 = 0x0001;
const CMD_DISPLAY_MODE: u16 = 0x0101;
const CMD_INPUT_SOURCE: u16 = 0x0105;
const CMD_SPLASH_LOAD: u16 = 0x0301;

/// Interface to the DLP2000 via Linux I2C.
pub struct Dlp {
    file: std::fs::File,
}

impl Dlp {
    /// Open the I2C bus and bind to the DLP2000 address.
    pub fn open(settings: &Settings) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&settings.i2c_bus)
            .map_err(|e| format!("open {}: {e}", settings.i2c_bus.display()))?;

        let ret = unsafe {
            libc::ioctl(
                file.as_raw_fd(),
                I2C_SLAVE,
                settings.dlp_i2c_addr as libc::c_int,
            )
        };
        if ret < 0 {
            return Err(format!(
                "I2C_SLAVE 0x{:02x} on {}: {}",
                settings.dlp_i2c_addr,
                settings.i2c_bus.display(),
                std::io::Error::last_os_error(),
            ));
        }

        logging::info(format!(
            "DLP2000 opened at {} addr=0x{:02x}",
            settings.i2c_bus.display(),
            settings.dlp_i2c_addr,
        ));

        Ok(Self { file })
    }

    /// Verify the DLPC2607 is responding and put it into external-pattern mode.
    ///
    /// TODO: full initialisation sequence:
    ///   1. Read CMD_DEVICE_ID and assert known value (0x2607).
    ///   2. Write CMD_INPUT_SOURCE = 0x00 (external parallel RGB).
    ///   3. Write CMD_DISPLAY_MODE = 0x00 (normal display).
    pub fn init(&mut self) -> Result<(), String> {
        logging::warn("DLP2000 init not yet implemented");
        Ok(())
    }

    /// Compute a slit-pattern image from `config` and load it onto the DLP.
    ///
    /// TODO:
    ///   1. Allocate a 640×480 pixel buffer (black background).
    ///   2. Convert `config.width_um` to pixels using the stored calibration.
    ///   3. Draw a white rectangle centred on (`offset_x_px`, `offset_y_px`).
    ///   4. Rotate the rectangle by `config.angle_deg`.
    ///   5. Scale brightness to `config.brightness`.
    ///   6. Upload the pattern via CMD_SPLASH_LOAD over I2C.
    pub fn set_slit(&mut self, config: &SlitConfig) -> Result<(), String> {
        logging::warn(format!(
            "DLP set_slit not yet implemented \
             (width={:.1}µm angle={:.1}° brightness={})",
            config.width_um, config.angle_deg, config.brightness,
        ));
        Ok(())
    }

    // ── low-level I2C helpers ─────────────────────────────────────────────────

    /// Write a DLPC2607 command packet: [cmd_hi, cmd_lo, data…].
    #[allow(dead_code)]
    fn write_command(&mut self, cmd: u16, data: &[u8]) -> Result<(), String> {
        let mut buf = vec![(cmd >> 8) as u8, (cmd & 0xff) as u8];
        buf.extend_from_slice(data);
        self.file
            .write_all(&buf)
            .map_err(|e| format!("I2C write cmd 0x{cmd:04x}: {e}"))
    }

    /// Read `len` bytes following a command.
    #[allow(dead_code)]
    fn read_response(&mut self, len: usize) -> Result<Vec<u8>, String> {
        use std::io::Read;
        let mut buf = vec![0u8; len];
        self.file
            .read_exact(&mut buf)
            .map_err(|e| format!("I2C read: {e}"))?;
        Ok(buf)
    }
}
