use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::thread;
use std::time::Duration;

use crate::config::Settings;
use crate::logging;

// ── TMC2209 register map ──────────────────────────────────────────────────────
// Source: TMC2209 datasheet Rev 1.09, Table 5.

const REG_GCONF: u8 = 0x00;
const REG_GSTAT: u8 = 0x01;
const REG_IHOLD_IRUN: u8 = 0x10;
const REG_VACTUAL: u8 = 0x22;
const REG_SGTHRS: u8 = 0x40;
const REG_SG_RESULT: u8 = 0x41;
const REG_CHOPCONF: u8 = 0x6c;
const REG_DRVSTATUS: u8 = 0x6f;

// Single-wire UART sync byte — always 0x05.
const SYNC: u8 = 0x05;
// Slave address — 0x00 when MS1/MS2 pins are low (default).
const SLAVE_ADDR: u8 = 0x00;

// GCONF value: UART mode enabled, StealthChop2 on, internal Rsense.
const GCONF_UART_STEALTHCHOP: u32 = 0x0000_01C0;
// CHOPCONF: 256 µstep, TOFF=3, standard SpreadCycle/StealthChop config.
const CHOPCONF_DEFAULT: u32 = 0x1000_0053;

/// Homing creep velocity (VACTUAL units ≈ pulses/s at 12 MHz clock).
/// ~200 RPM at 200 steps/rev full-step. Tune to taste.
const VACTUAL_HOME_CREEP: u32 = 100;

/// Delay between StallGuard polls during homing.
const HOME_POLL_MS: u64 = 10;

/// Maximum homing travel in poll cycles before giving up.
const HOME_TIMEOUT_CYCLES: u32 = 10_000;

pub struct Motion {
    file: std::fs::File,
    settings: Settings,
    pub position_steps: i32,
    pub homed: bool,
}

impl Motion {
    /// Open the UART device and configure it for TMC2209 single-wire comms.
    pub fn open(settings: &Settings) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOCTTY | libc::O_SYNC)
            .open(&settings.uart_device)
            .map_err(|e| format!("open {}: {e}", settings.uart_device.display()))?;

        configure_uart(file.as_raw_fd(), settings.uart_baud)?;

        logging::info(format!(
            "TMC2209 UART opened at {} baud={}",
            settings.uart_device.display(),
            settings.uart_baud,
        ));

        Ok(Self {
            file,
            settings: settings.clone(),
            position_steps: 0,
            homed: false,
        })
    }

    /// Configure the TMC2209 and verify it responds.
    ///
    /// Sequence:
    ///   1. Write GCONF  — enable UART mode + StealthChop.
    ///   2. Write CHOPCONF — 256 µstep, TOFF=3.
    ///   3. Write IHOLD_IRUN — configurable run/hold current.
    ///   4. Read GSTAT and clear any latched faults.
    ///   5. Read DRVSTATUS — confirm driver is not in fault.
    pub fn init(&mut self) -> Result<(), String> {
        // 1. GCONF
        self.write_reg(REG_GCONF, GCONF_UART_STEALTHCHOP)?;

        // 2. CHOPCONF
        self.write_reg(REG_CHOPCONF, CHOPCONF_DEFAULT)?;

        // 3. IHOLD_IRUN: [19:16]=IHOLDDELAY=6, [12:8]=IRUN, [4:0]=IHOLD
        let ihold_irun = (6u32 << 16)
            | ((self.settings.motor_run_current as u32) << 8)
            | (self.settings.motor_hold_current as u32);
        self.write_reg(REG_IHOLD_IRUN, ihold_irun)?;

        // 4. Clear GSTAT by writing back what we read.
        let gstat = self.read_reg(REG_GSTAT)?;
        if gstat != 0 {
            logging::warn(format!("TMC2209 GSTAT=0x{gstat:08x} — clearing latched faults"));
            self.write_reg(REG_GSTAT, gstat)?;
        }

        // 5. DRVSTATUS — bit 22 (otpw) or bit 23 (ot) indicate overtemp.
        let drvstatus = self.read_reg(REG_DRVSTATUS)?;
        if drvstatus & (1 << 23) != 0 {
            return Err("TMC2209 overtemperature fault (DRVSTATUS.ot)".to_string());
        }
        if drvstatus & (1 << 22) != 0 {
            logging::warn("TMC2209 overtemperature pre-warning (DRVSTATUS.otpw)");
        }

        logging::info(format!(
            "TMC2209 initialised — DRVSTATUS=0x{drvstatus:08x} \
             run={} hold={}",
            self.settings.motor_run_current, self.settings.motor_hold_current,
        ));

        Ok(())
    }

    /// Sensorless homing using StallGuard2.
    ///
    /// Sequence:
    ///   1. Set SGTHRS from config.
    ///   2. Drive VACTUAL in the negative direction (toward home).
    ///   3. Poll SG_RESULT every HOME_POLL_MS ms until it hits zero (stall).
    ///   4. Stop motor (VACTUAL=0).
    ///   5. Back off home_backoff_steps in the positive direction.
    ///   6. Zero position counter and mark homed.
    pub fn home(&mut self) -> Result<(), String> {
        logging::info("TMC2209 homing: setting StallGuard threshold");
        self.write_reg(REG_SGTHRS, self.settings.stallguard_threshold as u32)?;

        logging::info("TMC2209 homing: driving toward home");
        // Negative VACTUAL = motor turns in reverse.
        // VACTUAL is a signed 23-bit value; negative = two's complement.
        let vactual_neg = (-(VACTUAL_HOME_CREEP as i32)) as u32 & 0x00FF_FFFF;
        self.write_reg(REG_VACTUAL, vactual_neg)?;

        // Poll for stall
        let mut cycles = 0u32;
        loop {
            thread::sleep(Duration::from_millis(HOME_POLL_MS));
            let sg = self.read_reg(REG_SG_RESULT)?;
            if sg == 0 {
                logging::info("TMC2209 stall detected — stopping");
                break;
            }
            cycles += 1;
            if cycles >= HOME_TIMEOUT_CYCLES {
                self.write_reg(REG_VACTUAL, 0)?;
                return Err("homing timeout: no stall detected after maximum travel".to_string());
            }
        }

        // Stop
        self.write_reg(REG_VACTUAL, 0)?;
        thread::sleep(Duration::from_millis(50));

        // Back off
        self.move_velocity(self.settings.home_backoff_steps as i32)?;

        self.position_steps = 0;
        self.homed = true;
        logging::info("TMC2209 homing complete — position zeroed");
        Ok(())
    }

    /// Move by `steps` full steps (positive = forward, negative = reverse).
    /// Requires a successful prior home().
    pub fn move_steps(&mut self, steps: i32) -> Result<(), String> {
        if !self.homed {
            return Err("motor not homed; call home first".to_string());
        }
        self.move_velocity(steps)?;
        self.position_steps += steps;
        Ok(())
    }

    // ── velocity-based movement ───────────────────────────────────────────────

    /// Drive `steps` using VACTUAL, then stop.
    /// Rough timing: assumes ~200 steps/rev, VACTUAL_HOME_CREEP velocity.
    fn move_velocity(&mut self, steps: i32) -> Result<(), String> {
        if steps == 0 {
            return Ok(());
        }

        let vactual: u32 = if steps > 0 {
            VACTUAL_HOME_CREEP
        } else {
            (-(VACTUAL_HOME_CREEP as i32)) as u32 & 0x00FF_FFFF
        };

        self.write_reg(REG_VACTUAL, vactual)?;

        // Approximate time to travel `steps` at VACTUAL_HOME_CREEP.
        // VACTUAL units: (1/0.715) = ~1.4 full-steps/s per unit at 12 MHz.
        // Duration = |steps| / (VACTUAL * 1.4) seconds.
        let duration_ms =
            (steps.unsigned_abs() as u64 * 1000) / (VACTUAL_HOME_CREEP as u64 * 14 / 10).max(1);
        thread::sleep(Duration::from_millis(duration_ms.max(5)));

        self.write_reg(REG_VACTUAL, 0)?;
        thread::sleep(Duration::from_millis(10)); // settle
        Ok(())
    }

    // ── low-level UART register access ───────────────────────────────────────

    /// Write a 32-bit value to a TMC2209 register.
    /// Frame: SYNC(1) | RESERVED(1) | ADDR(1) | REG|0x80(1) | DATA(4) | CRC(1)
    fn write_reg(&mut self, reg: u8, value: u32) -> Result<(), String> {
        let mut pkt = [0u8; 8];
        pkt[0] = SYNC;
        pkt[1] = 0x00;
        pkt[2] = SLAVE_ADDR;
        pkt[3] = reg | 0x80;
        pkt[4] = ((value >> 24) & 0xff) as u8;
        pkt[5] = ((value >> 16) & 0xff) as u8;
        pkt[6] = ((value >> 8) & 0xff) as u8;
        pkt[7] = (value & 0xff) as u8;
        let crc = tmc_crc(&pkt);
        let mut buf = [0u8; 9];
        buf[..8].copy_from_slice(&pkt);
        buf[8] = crc;
        self.file
            .write_all(&buf)
            .map_err(|e| format!("UART write reg 0x{reg:02x}: {e}"))
    }

    /// Read a 32-bit value from a TMC2209 register.
    /// Send 4-byte read request, receive 8-byte reply frame.
    fn read_reg(&mut self, reg: u8) -> Result<u32, String> {
        // Send read request: SYNC | RESERVED | ADDR | REG | CRC
        let req = [SYNC, 0x00, SLAVE_ADDR, reg];
        let crc = tmc_crc(&req);
        let send = [req[0], req[1], req[2], req[3], crc];
        self.file
            .write_all(&send)
            .map_err(|e| format!("UART read-req reg 0x{reg:02x}: {e}"))?;

        // On single-wire UART the Pi will echo back its own TX bytes first;
        // drain the 5-byte echo before reading the 8-byte reply.
        let mut echo = [0u8; 5];
        self.file
            .read_exact(&mut echo)
            .map_err(|e| format!("UART echo drain reg 0x{reg:02x}: {e}"))?;

        // Read 8-byte reply frame
        let mut reply = [0u8; 8];
        self.file
            .read_exact(&mut reply)
            .map_err(|e| format!("UART read-reply reg 0x{reg:02x}: {e}"))?;

        // Validate reply CRC (last byte covers bytes 0–6)
        let expected_crc = tmc_crc(&reply[..7]);
        if reply[7] != expected_crc {
            return Err(format!(
                "TMC2209 CRC mismatch on reg 0x{reg:02x}: got 0x{:02x} expected 0x{:02x}",
                reply[7], expected_crc
            ));
        }

        Ok(((reply[3] as u32) << 24)
            | ((reply[4] as u32) << 16)
            | ((reply[5] as u32) << 8)
            | (reply[6] as u32))
    }
}

// ── CRC-8 ─────────────────────────────────────────────────────────────────────
// TMC2209 uses an 8-bit CRC with polynomial 0x07 (CRC-8/MAXIM variant).

fn tmc_crc(data: &[u8]) -> u8 {
    let mut crc: u8 = 0;
    for &byte in data {
        let mut b = byte;
        for _ in 0..8 {
            if (crc ^ b) & 0x01 != 0 {
                crc = (crc >> 1) ^ 0x07;
            } else {
                crc >>= 1;
            }
            b >>= 1;
        }
    }
    crc
}

// ── termios UART configuration ────────────────────────────────────────────────

fn configure_uart(fd: i32, baud: u32) -> Result<(), String> {
    let speed = baud_to_flag(baud).ok_or_else(|| format!("unsupported baud rate {baud}"))?;

    let mut tty: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut tty) } < 0 {
        return Err(format!("tcgetattr: {}", std::io::Error::last_os_error()));
    }

    unsafe {
        libc::cfsetispeed(&mut tty, speed);
        libc::cfsetospeed(&mut tty, speed);
    }

    // 8N1, raw mode, no flow control
    tty.c_cflag &= !(libc::PARENB | libc::CSTOPB | libc::CSIZE);
    tty.c_cflag |= libc::CS8 | libc::CREAD | libc::CLOCAL;
    tty.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ECHOE | libc::ISIG);
    tty.c_iflag &= !(libc::IXON | libc::IXOFF | libc::IXANY | libc::ICRNL);
    tty.c_oflag &= !libc::OPOST;
    // Non-blocking read with 100 ms timeout (VTIME in deciseconds).
    tty.c_cc[libc::VMIN] = 0;
    tty.c_cc[libc::VTIME] = 1;

    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &tty) } < 0 {
        return Err(format!("tcsetattr: {}", std::io::Error::last_os_error()));
    }

    Ok(())
}

fn baud_to_flag(baud: u32) -> Option<libc::speed_t> {
    match baud {
        9600 => Some(libc::B9600),
        19200 => Some(libc::B19200),
        38400 => Some(libc::B38400),
        57600 => Some(libc::B57600),
        115200 => Some(libc::B115200),
        230400 => Some(libc::B230400),
        _ => None,
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::tmc_crc;

    #[test]
    fn crc_known_vector() {
        // Write GCONF packet: 05 00 00 80 00 00 01 C0 → CRC=0x4A
        let data = [0x05u8, 0x00, 0x00, 0x80, 0x00, 0x00, 0x01, 0xC0];
        assert_eq!(tmc_crc(&data), 0x4A);
    }

    #[test]
    fn crc_read_request() {
        // Read GCONF request: 05 00 00 00 → CRC=0x54
        let data = [0x05u8, 0x00, 0x00, 0x00];
        assert_eq!(tmc_crc(&data), 0x54);
    }
}
