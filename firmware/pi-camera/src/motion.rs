//! TMC2209 stepper driver in step/dir mode via Linux GPIO sysfs.
//!
//! Pins (defaults match deploy/pi-camera.env):
//!   STEP  → GPIO4  (pin 7)
//!   DIR   → GPIO17 (pin 11)
//!   EN    → GPIO27 (pin 13, active LOW — pull low to enable)
//!   DIAG  → GPIO22 (pin 15, input — goes HIGH on stall)

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::config::Settings;
use crate::logging;

// ── GPIO sysfs paths ──────────────────────────────────────────────────────────

const GPIO_ROOT: &str = "/sys/class/gpio";

struct GpioPin {
    number: u32,
    value_path: PathBuf,
}

impl GpioPin {
    fn export(number: u32, direction: &str) -> Result<Self, String> {
        let gpio_dir = PathBuf::from(GPIO_ROOT).join(format!("gpio{number}"));

        // Export if not already exported.
        if !gpio_dir.exists() {
            fs::write(
                PathBuf::from(GPIO_ROOT).join("export"),
                number.to_string(),
            )
            .map_err(|e| format!("export GPIO{number}: {e}"))?;

            // Wait for the sysfs entry to appear.
            let mut waited = 0u32;
            while !gpio_dir.exists() {
                thread::sleep(Duration::from_millis(10));
                waited += 10;
                if waited > 500 {
                    return Err(format!("GPIO{number} sysfs entry did not appear"));
                }
            }
        }

        fs::write(gpio_dir.join("direction"), direction)
            .map_err(|e| format!("set GPIO{number} direction={direction}: {e}"))?;

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
        // Best-effort unexport on cleanup.
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
    /// Delay between step pulses — controls speed.
    step_delay: Duration,
    pub position_steps: i32,
    pub homed: bool,
}

impl Motion {
    pub fn open(settings: &Settings) -> Result<Self, String> {
        let step = GpioPin::export(settings.gpio_step, "out")?;
        let dir  = GpioPin::export(settings.gpio_dir,  "out")?;
        let en   = GpioPin::export(settings.gpio_en,   "out")?;
        let diag = GpioPin::export(settings.gpio_diag, "in")?;

        // Start with motor disabled (EN active low → set HIGH to disable).
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

    /// Enable the motor driver (EN pulled LOW).
    pub fn enable(&self) -> Result<(), String> {
        self.en.set(false)
    }

    /// Disable the motor driver (EN pulled HIGH — motor coils de-energised).
    pub fn disable(&self) -> Result<(), String> {
        self.en.set(true)
    }

    /// Move `steps` steps. Positive = forward, negative = reverse.
    pub fn move_steps(&mut self, steps: i32) -> Result<(), String> {
        if !self.homed {
            return Err("motor not homed; call home first".to_string());
        }
        self.step_move(steps)?;
        self.position_steps += steps;
        Ok(())
    }

    /// Homing: drive toward the endstop (DIAG pin) until stall is detected,
    /// then back off and zero the position counter.
    pub fn home(&mut self, settings: &Settings) -> Result<(), String> {
        logging::info("Homing: enabling motor and driving toward endstop");
        self.enable()?;

        // Drive in the homing direction (negative = toward home).
        self.dir.set(false)?;

        let max_steps = settings.home_max_steps as i32;
        let mut steps_taken = 0i32;

        loop {
            // Check DIAG before each step — TMC2209 raises DIAG on stall.
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
                     check wiring and DIAG pin"
                ));
            }
        }

        // Stop and let the motor settle.
        thread::sleep(Duration::from_millis(50));

        // Back off from the endstop.
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

    // ── low-level helpers ─────────────────────────────────────────────────────

    /// Send `steps` pulses in the direction set by `steps` sign.
    fn step_move(&mut self, steps: i32) -> Result<(), String> {
        if steps == 0 {
            return Ok(());
        }
        self.enable()?;
        self.dir.set(steps > 0)?;
        // Small settle time after changing direction.
        thread::sleep(Duration::from_micros(10));
        for _ in 0..steps.unsigned_abs() {
            self.pulse_step()?;
        }
        Ok(())
    }

    /// One step pulse: HIGH for step_delay, then LOW for step_delay.
    fn pulse_step(&self) -> Result<(), String> {
        self.step.set(true)?;
        thread::sleep(self.step_delay);
        self.step.set(false)?;
        thread::sleep(self.step_delay);
        Ok(())
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #[test]
    fn step_delay_from_micros() {
        let d = std::time::Duration::from_micros(500);
        assert_eq!(d.as_micros(), 500);
    }
}
