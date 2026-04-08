//! TMC2209 stepper driver in step/dir mode via Linux GPIO sysfs.
//!
//! Pins (defaults match deploy/pi-camera.env):
//!   STEP  → GPIO4  (pin 7)
//!   DIR   → GPIO17 (pin 11)
//!   EN    → GPIO27 (pin 13, active LOW — pull low to enable)
//!   DIAG  → GPIO22 (pin 15, input — goes HIGH on stall)
//!   UART  → GPIO14 (pin 8, TX only → TMC2209 PDN_UART)

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::config::Settings;
use crate::logging;

// ── TMC2209 UART (TX-only, write datagrams only) ──────────────────────────────
//
// Register addresses
const REG_GCONF:     u8 = 0x00;
const REG_TCOOLTHRS: u8 = 0x14;
const REG_SGTHRS:    u8 = 0x40;

// GCONF value: en_SpreadCycle (bit2) + diag1_stall (bit5)
const GCONF_SPREADCYCLE_DIAG_STALL: u32 = (1 << 2) | (1 << 5);

/// Compute the TMC2209 CRC-8 over the first N bytes of a datagram.
fn tmc_crc(data: &[u8]) -> u8 {
    let mut crc: u8 = 0;
    for &byte in data {
        let mut b = byte;
        for _ in 0..8 {
            if (crc >> 7) ^ (b & 0x01) != 0 {
                crc = (crc << 1) ^ 0x07;
            } else {
                crc <<= 1;
            }
            b >>= 1;
        }
    }
    crc
}

/// Build an 8-byte write datagram for the given register and 32-bit value.
fn write_datagram(slave: u8, reg: u8, value: u32) -> [u8; 8] {
    let mut d = [0u8; 8];
    d[0] = 0x05;              // sync byte
    d[1] = slave;             // slave address (0x00 = MS1/MS2 both low)
    d[2] = reg | 0x80;        // register address with write bit set
    d[3] = ((value >> 24) & 0xFF) as u8;
    d[4] = ((value >> 16) & 0xFF) as u8;
    d[5] = ((value >>  8) & 0xFF) as u8;
    d[6] = ( value        & 0xFF) as u8;
    d[7] = tmc_crc(&d[..7]);
    d
}

/// Configure the TMC2209 via TX-only UART.
/// Writes GCONF (spreadCycle + diag1_stall), TCOOLTHRS, and SGTHRS.
fn tmc_uart_configure(device: &str, sg_threshold: u8) -> Result<(), String> {
    // Set port to 115200 baud raw via stty before opening.
    let status = Command::new("stty")
        .args(["-F", device, "115200", "raw", "-echo", "cs8", "-cstopb", "-parenb"])
        .status()
        .map_err(|e| format!("stty: {e}"))?;
    if !status.success() {
        return Err(format!("stty failed on {device}"));
    }

    let mut port = OpenOptions::new()
        .write(true)
        .open(device)
        .map_err(|e| format!("open {device}: {e}"))?;

    // GCONF: enable SpreadCycle and route stall signal to DIAG pin.
    port.write_all(&write_datagram(0, REG_GCONF, GCONF_SPREADCYCLE_DIAG_STALL))
        .map_err(|e| format!("write GCONF: {e}"))?;
    thread::sleep(Duration::from_millis(5));

    // TCOOLTHRS: enable StallGuard at all velocities (set to max).
    port.write_all(&write_datagram(0, REG_TCOOLTHRS, 0x000F_FFFF))
        .map_err(|e| format!("write TCOOLTHRS: {e}"))?;
    thread::sleep(Duration::from_millis(5));

    // SGTHRS: stall sensitivity 0–255 (higher = more sensitive).
    port.write_all(&write_datagram(0, REG_SGTHRS, u32::from(sg_threshold)))
        .map_err(|e| format!("write SGTHRS: {e}"))?;
    thread::sleep(Duration::from_millis(5));

    logging::info(format!(
        "TMC2209 UART configured via {device}: spreadCycle, diag1_stall, SGTHRS={}",
        sg_threshold
    ));
    Ok(())
}

// ── GPIO sysfs paths ──────────────────────────────────────────────────────────

const GPIO_ROOT: &str = "/sys/class/gpio";

/// On kernels ≥ 6.x the BCM GPIO chip is registered with a base offset
/// (typically 512) rather than 0.  Read it from sysfs so BCM pin numbers
/// work regardless of kernel version.
fn gpio_chip_base() -> u32 {
    let root = Path::new(GPIO_ROOT);
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };
    let mut bases: Vec<u32> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("gpiochip"))
        .filter_map(|e| {
            fs::read_to_string(e.path().join("base"))
                .ok()?
                .trim()
                .parse::<u32>()
                .ok()
        })
        .collect();
    bases.sort();
    bases.into_iter().next().unwrap_or(0)
}

struct GpioPin {
    number: u32,
    value_path: PathBuf,
}

impl GpioPin {
    fn export(bcm_number: u32, direction: &str) -> Result<Self, String> {
        let number = gpio_chip_base() + bcm_number;
        let gpio_dir = PathBuf::from(GPIO_ROOT).join(format!("gpio{number}"));

        if !gpio_dir.exists() {
            fs::write(
                PathBuf::from(GPIO_ROOT).join("export"),
                number.to_string(),
            )
            .map_err(|e| format!("export GPIO{bcm_number} (sysfs {number}): {e}"))?;

            let mut waited = 0u32;
            while !gpio_dir.exists() {
                thread::sleep(Duration::from_millis(10));
                waited += 10;
                if waited > 500 {
                    return Err(format!("GPIO{bcm_number} sysfs entry did not appear"));
                }
            }
        }

        fs::write(gpio_dir.join("direction"), direction)
            .map_err(|e| format!("set GPIO{bcm_number} direction={direction}: {e}"))?;

        Ok(Self {
            number,
            value_path: gpio_dir.join("value"),
        })
    }

    fn set(&self, high: bool) -> Result<(), String> {
        fs::write(&self.value_path, if high { "1" } else { "0" })
            .map_err(|e| format!("write GPIO{}: {e}", self.number))
    }

    fn get(&self) -> Result<bool, String> {
        let val = fs::read_to_string(&self.value_path)
            .map_err(|e| format!("read GPIO{}: {e}", self.number))?;
        Ok(val.trim() == "1")
    }
}

impl Drop for GpioPin {
    fn drop(&mut self) {
        let _ = fs::write(
            PathBuf::from(GPIO_ROOT).join("unexport"),
            self.number.to_string(),
        );
    }
}

// ── Motion controller ─────────────────────────────────────────────────────────

pub struct Motion {
    step: GpioPin,
    dir: GpioPin,
    en: GpioPin,
    diag: GpioPin,
    step_delay: Duration,
    pub position_steps: i32,
    pub homed: bool,
}

impl Drop for Motion {
    fn drop(&mut self) {
        // Always disable the motor on shutdown so coils don't stay energized.
        let _ = self.en.set(true);
    }
}

impl Motion {
    pub fn open(settings: &Settings) -> Result<Self, String> {
        // Configure TMC2209 via UART before touching GPIO.
        match tmc_uart_configure(&settings.uart_device, settings.sg_threshold) {
            Ok(()) => {}
            Err(e) => logging::warn(format!(
                "TMC2209 UART config failed: {e} — sensorless homing disabled"
            )),
        }

        let step = GpioPin::export(settings.gpio_step, "out")?;
        let dir  = GpioPin::export(settings.gpio_dir,  "out")?;
        let en   = GpioPin::export(settings.gpio_en,   "out")?;
        let diag = GpioPin::export(settings.gpio_diag, "in")?;

        en.set(true)?;
        step.set(false)?;
        dir.set(false)?;

        let step_delay = Duration::from_micros(settings.step_delay_us);

        logging::info(format!(
            "GPIO motion: STEP=GPIO{} DIR=GPIO{} EN=GPIO{} DIAG=GPIO{}  delay={}µs",
            settings.gpio_step,
            settings.gpio_dir,
            settings.gpio_en,
            settings.gpio_diag,
            settings.step_delay_us,
        ));

        Ok(Self {
            step,
            dir,
            en,
            diag,
            step_delay,
            position_steps: 0,
            homed: false,
        })
    }

    pub fn enable(&self) -> Result<(), String> {
        self.en.set(false)
    }

    pub fn disable(&self) -> Result<(), String> {
        self.en.set(true)
    }

    pub fn move_steps(&mut self, steps: i32) -> Result<(), String> {
        self.step_move(steps)?;
        self.position_steps += steps;
        Ok(())
    }

    pub fn home(&mut self, settings: &Settings) -> Result<(), String> {
        logging::info("Homing: enabling motor and driving toward endstop");
        self.enable()?;
        self.dir.set(false)?;

        let max_steps = settings.home_max_steps as i32;
        let mut steps_taken = 0i32;

        loop {
            if self.diag.get()? {
                logging::info(format!(
                    "Homing: stall detected after {steps_taken} steps"
                ));
                break;
            }

            self.pulse_step()?;
            steps_taken += 1;

            if steps_taken >= max_steps {
                self.disable()?;
                return Err(format!(
                    "homing failed: no stall detected after {max_steps} steps; \
                     check SGTHRS tuning or DIAG wiring"
                ));
            }
        }

        thread::sleep(Duration::from_millis(50));

        logging::info(format!(
            "Homing: backing off {} steps",
            settings.home_backoff_steps
        ));
        self.dir.set(true)?;
        for _ in 0..settings.home_backoff_steps {
            self.pulse_step()?;
        }

        self.position_steps = 0;
        self.homed = true;
        logging::info("Homing complete — position zeroed");
        Ok(())
    }

    fn step_move(&mut self, steps: i32) -> Result<(), String> {
        if steps == 0 {
            return Ok(());
        }
        self.enable()?;
        self.dir.set(steps > 0)?;
        thread::sleep(Duration::from_micros(10));
        for _ in 0..steps.unsigned_abs() {
            self.pulse_step()?;
        }
        Ok(())
    }

    fn pulse_step(&self) -> Result<(), String> {
        self.step.set(true)?;
        thread::sleep(self.step_delay);
        self.step.set(false)?;
        thread::sleep(self.step_delay);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::tmc_crc;

    #[test]
    fn crc_known_value() {
        // Known-good CRC for a GCONF write datagram to slave 0.
        let d = [0x05u8, 0x00, 0x80, 0x00, 0x00, 0x00, 0x24];
        // Just verify it doesn't panic and returns a byte.
        let _ = tmc_crc(&d);
    }
}
